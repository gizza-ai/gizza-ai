//! merge-conflict-resolver core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps, no I/O, no clock: the same input always
//! produces the same bytes.
//!
//! Parses the conflict markers Git leaves in a working-tree file — the standard
//! two-way style
//!
//! ```text
//! <<<<<<< HEAD
//! ours
//! =======
//! theirs
//! >>>>>>> feature/login
//! ```
//!
//! and the three-way `diff3` / `zdiff3` style, which adds a common-ancestor section
//! introduced by `|||||||` between the two sides — then rewrites each conflict with the
//! chosen side (ours, theirs, both in either order, the common ancestor, or left
//! untouched), lists the conflicts as a numbered inventory, renders the two sides as
//! aligned text columns, or reports everything as JSON.
//!
//! Everything outside a conflict block is preserved byte for byte, including the input's
//! line ending (LF or CRLF) and whether it ended with a newline.

use serde_json::{json, Value};

/// Hard cap on the pasted text — 1 MB. Anything bigger is a repository dump rather
/// than one conflicted file, and the page keeps the whole thing in wasm memory.
pub const MAX_INPUT_BYTES: usize = 1_000_000;

/// Widest column used by the `sides` view before a line is truncated with an ellipsis.
const SIDES_MAX_COL: usize = 80;

/// Minimum run length for a marker line. Git writes exactly seven characters, but
/// longer runs are accepted so hand-widened markers still parse.
const MARKER_RUN: usize = 7;

/// What to do with one conflict block.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Strategy {
    /// Keep the first side — the version already on the current branch.
    Ours,
    /// Keep the second side — the version being merged in.
    Theirs,
    /// Keep both sides, ours first.
    Both,
    /// Keep both sides, theirs first.
    BothTheirsFirst,
    /// Keep the `|||||||` common-ancestor section (diff3 / zdiff3 input only).
    Base,
    /// Leave the block exactly as pasted, markers included.
    Keep,
}

impl Strategy {
    fn name(self) -> &'static str {
        match self {
            Strategy::Ours => "ours",
            Strategy::Theirs => "theirs",
            Strategy::Both => "both",
            Strategy::BothTheirsFirst => "both-theirs-first",
            Strategy::Base => "base",
            Strategy::Keep => "keep",
        }
    }
}

const STRATEGY_VALUES: &str = "ours, theirs, both, both-theirs-first, base, keep";

fn parse_strategy(raw: &str) -> Result<Strategy, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" | "ours" => Ok(Strategy::Ours),
        "theirs" => Ok(Strategy::Theirs),
        "both" | "both-ours-first" => Ok(Strategy::Both),
        "both-theirs-first" => Ok(Strategy::BothTheirsFirst),
        "base" => Ok(Strategy::Base),
        "keep" => Ok(Strategy::Keep),
        other => Err(format!(
            "unknown strategy '{other}': expected one of {STRATEGY_VALUES}"
        )),
    }
}

/// One parsed conflict block.
struct Conflict {
    /// 1-based position in the pasted text.
    index: usize,
    /// 1-based line number of the `<<<<<<<` line.
    start_line: usize,
    /// 1-based line number of the `>>>>>>>` line.
    end_line: usize,
    ours_label: String,
    base_label: Option<String>,
    theirs_label: String,
    ours: Vec<String>,
    base: Option<Vec<String>>,
    theirs: Vec<String>,
    /// The block verbatim, markers included, so `keep` round-trips exactly.
    raw: Vec<String>,
}

/// The pasted text as a sequence of untouched lines and conflict blocks.
enum Piece {
    Line(String),
    Conflict(usize),
}

/// Recognises a marker line: at least seven copies of `ch`, then end of line or a
/// space. Returns the trailing label (the branch name Git appends), trimmed.
fn marker_label(line: &str, ch: char) -> Option<String> {
    let run = line.chars().take_while(|c| *c == ch).count();
    if run < MARKER_RUN {
        return None;
    }
    let rest: String = line.chars().skip(run).collect();
    if rest.is_empty() || rest.starts_with(' ') {
        Some(rest.trim().to_string())
    } else {
        None
    }
}

struct Parsed {
    pieces: Vec<Piece>,
    conflicts: Vec<Conflict>,
    crlf: bool,
    trailing_newline: bool,
}

/// Splits the text into lines and conflict blocks, or explains where the markers
/// stopped making sense.
fn parse(text: &str) -> Result<Parsed, String> {
    let crlf = text.contains("\r\n");
    let trailing_newline = text.ends_with('\n');
    let body = text.strip_suffix('\n').unwrap_or(text);
    let lines: Vec<&str> = body
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();

    let mut pieces = Vec::new();
    let mut conflicts: Vec<Conflict> = Vec::new();

    // State of the block currently being read, if any.
    let mut open: Option<Conflict> = None;
    // Which section of the open block the reader is in.
    let mut section = 0u8; // 0 = ours, 1 = base, 2 = theirs

    for (i, raw_line) in lines.iter().enumerate() {
        let lineno = i + 1;
        let line = *raw_line;
        let start = marker_label(line, '<');
        let base = marker_label(line, '|');
        let sep = marker_label(line, '=');
        let end = marker_label(line, '>');

        match open.as_mut() {
            None => {
                if let Some(label) = start {
                    open = Some(Conflict {
                        index: conflicts.len() + 1,
                        start_line: lineno,
                        end_line: 0,
                        ours_label: label,
                        base_label: None,
                        theirs_label: String::new(),
                        ours: Vec::new(),
                        base: None,
                        theirs: Vec::new(),
                        raw: vec![line.to_string()],
                    });
                    section = 0;
                } else if base.is_some() {
                    return Err(format!(
                        "stray '|||||||' marker at line {lineno}: no conflict was opened with '<<<<<<<' before it"
                    ));
                } else if end.is_some() {
                    return Err(format!(
                        "stray '>>>>>>>' marker at line {lineno}: no conflict was opened with '<<<<<<<' before it"
                    ));
                } else {
                    // A bare '=======' outside a conflict is ordinary text (a Markdown
                    // setext underline, for instance), so it is left alone.
                    pieces.push(Piece::Line(line.to_string()));
                }
            }
            Some(c) => {
                let opened = c.start_line;
                if start.is_some() {
                    return Err(format!(
                        "nested '<<<<<<<' at line {lineno}: the conflict opened at line {opened} is still unterminated"
                    ));
                }
                c.raw.push(line.to_string());
                if let Some(label) = base {
                    if section != 0 {
                        return Err(format!(
                            "unexpected '|||||||' at line {lineno}: the conflict opened at line {opened} already passed its common-ancestor section"
                        ));
                    }
                    c.base_label = Some(label);
                    c.base = Some(Vec::new());
                    section = 1;
                } else if sep.is_some() {
                    if section == 2 {
                        return Err(format!(
                            "second '=======' at line {lineno}: the conflict opened at line {opened} already has a separator"
                        ));
                    }
                    section = 2;
                } else if let Some(label) = end {
                    if section != 2 {
                        return Err(format!(
                            "'>>>>>>>' at line {lineno} closes the conflict opened at line {opened}, but no '=======' separator was seen"
                        ));
                    }
                    c.theirs_label = label;
                    c.end_line = lineno;
                    let done = open.take().expect("a block is open");
                    pieces.push(Piece::Conflict(conflicts.len()));
                    conflicts.push(done);
                } else {
                    let owned = line.to_string();
                    match section {
                        0 => c.ours.push(owned),
                        1 => c.base.as_mut().expect("base section started").push(owned),
                        _ => c.theirs.push(owned),
                    }
                }
            }
        }
    }

    if let Some(c) = open {
        return Err(format!(
            "unterminated conflict: '<<<<<<<' at line {} has no matching '>>>>>>>'",
            c.start_line
        ));
    }

    Ok(Parsed {
        pieces,
        conflicts,
        crlf,
        trailing_newline,
    })
}

/// Parses one `choices` span (`3`, `3-5`, `4-`, `-2`, `all`) into an inclusive range.
fn parse_span(span: &str, count: usize, entry: &str) -> Result<(usize, usize), String> {
    let span = span.trim();
    if span.eq_ignore_ascii_case("all") {
        return Ok((1, count));
    }
    let bad = || {
        format!(
            "invalid conflict selector '{span}' in choice '{entry}': expected a number ('2'), a range ('3-5', '4-', '-2') or 'all'"
        )
    };
    let num = |s: &str| -> Result<usize, String> { s.trim().parse::<usize>().map_err(|_| bad()) };
    let (lo, hi) = match span.split_once('-') {
        None => {
            let n = num(span)?;
            (n, n)
        }
        Some((a, b)) => {
            let lo = if a.trim().is_empty() { 1 } else { num(a)? };
            let hi = if b.trim().is_empty() { count } else { num(b)? };
            (lo, hi)
        }
    };
    if lo == 0 || hi == 0 {
        return Err(format!(
            "invalid conflict selector '{span}' in choice '{entry}': conflicts are numbered from 1"
        ));
    }
    if lo > hi {
        return Err(format!(
            "invalid conflict range '{span}' in choice '{entry}': {lo} is after {hi}"
        ));
    }
    if hi > count {
        return Err(format!(
            "choice '{entry}' refers to conflict {hi}, but the input has {count} conflict{}",
            if count == 1 { "" } else { "s" }
        ));
    }
    Ok((lo, hi))
}

/// Builds the per-conflict strategy table: the bulk `strategy`, then every `choices`
/// override applied in order (a later entry wins over an earlier overlapping one).
fn plan(conflicts: &[Conflict], strategy: Strategy, choices: &str) -> Result<Vec<Strategy>, String> {
    let count = conflicts.len();
    let mut plan = vec![strategy; count];
    let trimmed = choices.trim();
    if trimmed.is_empty() {
        return Ok(plan);
    }
    if count == 0 {
        return Err(
            "choices were given, but the input has no conflict markers to apply them to".into(),
        );
    }
    for entry in trimmed.split([',', ';']) {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let (span, value) = entry.split_once('=').ok_or_else(|| {
            format!(
                "invalid choice '{entry}': expected <conflict>=<strategy>, e.g. '2=theirs' or '3-5=both'"
            )
        })?;
        let strat = parse_strategy(value).map_err(|e| format!("in choice '{entry}': {e}"))?;
        if value.trim().is_empty() {
            return Err(format!(
                "invalid choice '{entry}': missing strategy after '=' (expected one of {STRATEGY_VALUES})"
            ));
        }
        let (lo, hi) = parse_span(span, count, entry)?;
        for slot in plan.iter_mut().take(hi).skip(lo - 1) {
            *slot = strat;
        }
    }
    Ok(plan)
}

/// The lines a conflict collapses to under its chosen strategy.
fn replacement<'a>(c: &'a Conflict, strat: Strategy) -> Result<Vec<&'a String>, String> {
    let both = |first: &'a [String], second: &'a [String]| first.iter().chain(second.iter()).collect();
    Ok(match strat {
        Strategy::Ours => c.ours.iter().collect(),
        Strategy::Theirs => c.theirs.iter().collect(),
        Strategy::Both => both(&c.ours, &c.theirs),
        Strategy::BothTheirsFirst => both(&c.theirs, &c.ours),
        Strategy::Keep => c.raw.iter().collect(),
        Strategy::Base => match c.base.as_ref() {
            Some(b) => b.iter().collect(),
            None => {
                return Err(format!(
                    "conflict [{}] at line {} has no '|||||||' common-ancestor section, so strategy 'base' cannot be applied — re-create the conflict with the diff3 or zdiff3 conflict style to get one",
                    c.index, c.start_line
                ))
            }
        },
    })
}

fn join(lines: &[String], crlf: bool, trailing_newline: bool) -> String {
    let nl = if crlf { "\r\n" } else { "\n" };
    let mut out = lines.join(nl);
    if trailing_newline {
        out.push_str(nl);
    }
    out
}

fn labelled(kind: &str, label: &str) -> String {
    if label.is_empty() {
        kind.to_string()
    } else {
        format!("{kind} \"{label}\"")
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn header(conflicts: &[Conflict], plan: &[Strategy]) -> String {
    if conflicts.is_empty() {
        return "0 conflicts · no conflict markers found".to_string();
    }
    let kept = plan.iter().filter(|s| **s == Strategy::Keep).count();
    let resolved = plan.len() - kept;
    let counts = plural(conflicts.len(), "conflict", "conflicts");
    if kept == 0 {
        format!("{counts} · {resolved} resolved")
    } else {
        format!("{counts} · {resolved} resolved, {kept} kept")
    }
}

fn render_list(conflicts: &[Conflict], plan: &[Strategy]) -> String {
    let mut out = vec![header(conflicts, plan)];
    if !conflicts.is_empty() {
        out.push(String::new());
    }
    for (c, strat) in conflicts.iter().zip(plan.iter()) {
        let mut parts = vec![format!(
            "{} {}",
            labelled("ours", &c.ours_label),
            plural(c.ours.len(), "line", "lines")
        )];
        if let Some(base) = c.base.as_ref() {
            parts.push(format!(
                "{} {}",
                labelled("base", c.base_label.as_deref().unwrap_or("")),
                plural(base.len(), "line", "lines")
            ));
        }
        parts.push(format!(
            "{} {}",
            labelled("theirs", &c.theirs_label),
            plural(c.theirs.len(), "line", "lines")
        ));
        out.push(format!(
            "[{}] lines {}-{} · {} → {}",
            c.index,
            c.start_line,
            c.end_line,
            parts.join(" · "),
            strat.name()
        ));
    }
    out.join("\n")
}

/// Trims a cell to `width` display characters, marking the cut with an ellipsis.
fn cell(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    let mut s: String = text.chars().take(width.saturating_sub(1)).collect();
    s.push('…');
    s
}

fn render_sides(conflicts: &[Conflict], plan: &[Strategy]) -> String {
    let mut out = vec![header(conflicts, plan)];
    for (c, strat) in conflicts.iter().zip(plan.iter()) {
        let left_head = labelled("ours", &c.ours_label);
        let right_head = labelled("theirs", &c.theirs_label);
        let width = |head: &str, lines: &[String]| {
            lines
                .iter()
                .map(|l| l.chars().count())
                .chain(std::iter::once(head.chars().count()))
                .max()
                .unwrap_or(0)
                .min(SIDES_MAX_COL)
        };
        let lw = width(&left_head, &c.ours);
        let rw = width(&right_head, &c.theirs);

        out.push(String::new());
        out.push(format!(
            "[{}] lines {}-{} → {}",
            c.index,
            c.start_line,
            c.end_line,
            strat.name()
        ));
        let pad = |s: &str, w: usize| format!("{:<width$}", cell(s, w), width = w);
        out.push(format!("{} | {}", pad(&left_head, lw), cell(&right_head, rw)));
        out.push(format!("{}-+-{}", "-".repeat(lw), "-".repeat(rw)));
        let rows = c.ours.len().max(c.theirs.len());
        for r in 0..rows {
            let left = c.ours.get(r).map(String::as_str).unwrap_or("");
            let right = c.theirs.get(r).map(String::as_str).unwrap_or("");
            out.push(format!("{} | {}", pad(left, lw), cell(right, rw)).trim_end().to_string());
        }
        if let Some(base) = c.base.as_ref() {
            out.push(labelled("base", c.base_label.as_deref().unwrap_or("")));
            for line in base {
                out.push(format!("  {}", cell(line, SIDES_MAX_COL)));
            }
        }
    }
    out.join("\n")
}

fn render_json(parsed: &Parsed, plan: &[Strategy], resolved: &str) -> String {
    let conflicts: Vec<Value> = parsed
        .conflicts
        .iter()
        .zip(plan.iter())
        .map(|(c, strat)| {
            json!({
                "index": c.index,
                "start_line": c.start_line,
                "end_line": c.end_line,
                "ours_label": c.ours_label,
                "base_label": c.base_label,
                "theirs_label": c.theirs_label,
                "ours": c.ours,
                "base": c.base,
                "theirs": c.theirs,
                "strategy": strat.name(),
            })
        })
        .collect();
    let kept = plan.iter().filter(|s| **s == Strategy::Keep).count();
    let doc = json!({
        "conflict_count": parsed.conflicts.len(),
        "resolved_count": plan.len() - kept,
        "kept_count": kept,
        "conflict_style": if parsed.conflicts.iter().any(|c| c.base.is_some()) { "diff3" } else { "merge" },
        "line_ending": if parsed.crlf { "crlf" } else { "lf" },
        "conflicts": conflicts,
        "resolved": resolved,
    });
    serde_json::to_string_pretty(&doc).unwrap_or_else(|e| e.to_string())
}

/// Resolve (or inspect) the conflict markers in `text`.
///
/// - `strategy`: the bulk choice applied to every conflict — see [`STRATEGY_VALUES`].
/// - `choices`: per-conflict overrides, e.g. `"2=theirs, 3-5=both"`.
/// - `output`: `resolved` (default) | `list` | `sides` | `json`.
/// - `strict`: error when the text holds no conflict markers, or when any conflict is
///   left unresolved.
pub fn resolve(
    text: &str,
    strategy: &str,
    choices: &str,
    output: &str,
    strict: bool,
) -> Result<String, String> {
    if text.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input too large: {} bytes, limit is {MAX_INPUT_BYTES} bytes (1 MB)",
            text.len()
        ));
    }
    if text.trim().is_empty() {
        return Err("empty input: paste the file text that still contains the conflict markers".into());
    }
    let output_mode = output.trim().to_ascii_lowercase();
    let output_mode = if output_mode.is_empty() {
        "resolved"
    } else {
        output_mode.as_str()
    };
    if !matches!(output_mode, "resolved" | "list" | "sides" | "json") {
        return Err(format!(
            "unknown output '{output}': expected one of resolved, list, sides, json"
        ));
    }

    let bulk = parse_strategy(strategy)?;
    let parsed = parse(text)?;
    let plan = plan(&parsed.conflicts, bulk, choices)?;

    if strict {
        if parsed.conflicts.is_empty() {
            return Err(
                "strict mode: no conflict markers ('<<<<<<<' … '>>>>>>>') were found in the input"
                    .into(),
            );
        }
        if let Some((c, _)) = parsed
            .conflicts
            .iter()
            .zip(plan.iter())
            .find(|(_, s)| **s == Strategy::Keep)
        {
            return Err(format!(
                "strict mode: conflict [{}] at line {} is left unresolved by strategy 'keep'",
                c.index, c.start_line
            ));
        }
    }

    let mut lines: Vec<String> = Vec::new();
    for piece in &parsed.pieces {
        match piece {
            Piece::Line(l) => lines.push(l.clone()),
            Piece::Conflict(i) => {
                let c = &parsed.conflicts[*i];
                for l in replacement(c, plan[*i])? {
                    lines.push(l.clone());
                }
            }
        }
    }
    let resolved = join(&lines, parsed.crlf, parsed.trailing_newline);

    Ok(match output_mode {
        "list" => render_list(&parsed.conflicts, &plan),
        "sides" => render_sides(&parsed.conflicts, &plan),
        "json" => render_json(&parsed, &plan, &resolved),
        _ => resolved,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_WAY: &str = "const port = 1;\n\
<<<<<<< HEAD\n\
const host = \"0.0.0.0\";\n\
=======\n\
const host = \"127.0.0.1\";\n\
>>>>>>> feature/local\n\
start();\n";

    const DIFF3: &str = "<<<<<<< HEAD\n\
a-ours\n\
||||||| merged common ancestors\n\
a-base\n\
=======\n\
a-theirs\n\
>>>>>>> topic\n";

    #[test]
    fn ours_is_the_default_and_drops_the_markers() {
        let out = resolve(TWO_WAY, "", "", "", false).unwrap();
        assert_eq!(
            out,
            "const port = 1;\nconst host = \"0.0.0.0\";\nstart();\n"
        );
    }

    #[test]
    fn theirs_both_and_reversed_both() {
        assert_eq!(
            resolve(TWO_WAY, "theirs", "", "resolved", false).unwrap(),
            "const port = 1;\nconst host = \"127.0.0.1\";\nstart();\n"
        );
        assert_eq!(
            resolve(TWO_WAY, "both", "", "resolved", false).unwrap(),
            "const port = 1;\nconst host = \"0.0.0.0\";\nconst host = \"127.0.0.1\";\nstart();\n"
        );
        assert_eq!(
            resolve(TWO_WAY, "both-theirs-first", "", "resolved", false).unwrap(),
            "const port = 1;\nconst host = \"127.0.0.1\";\nconst host = \"0.0.0.0\";\nstart();\n"
        );
    }

    #[test]
    fn keep_round_trips_the_block_verbatim() {
        assert_eq!(resolve(TWO_WAY, "keep", "", "resolved", false).unwrap(), TWO_WAY);
    }

    #[test]
    fn diff3_base_section_is_parsed_and_selectable() {
        assert_eq!(resolve(DIFF3, "base", "", "resolved", false).unwrap(), "a-base\n");
        assert_eq!(resolve(DIFF3, "ours", "", "resolved", false).unwrap(), "a-ours\n");
        let list = resolve(DIFF3, "ours", "", "list", false).unwrap();
        assert!(list.contains("base \"merged common ancestors\" 1 line"), "{list}");
    }

    #[test]
    fn base_without_a_diff3_section_is_a_located_error() {
        let err = resolve(TWO_WAY, "base", "", "resolved", false).unwrap_err();
        assert!(err.contains("conflict [1] at line 2"), "{err}");
        assert!(err.contains("'|||||||'"), "{err}");
    }

    #[test]
    fn per_conflict_choices_override_the_bulk_strategy() {
        let text = "<<<<<<<\n1o\n=======\n1t\n>>>>>>>\nmid\n<<<<<<<\n2o\n=======\n2t\n>>>>>>>\n";
        assert_eq!(
            resolve(text, "ours", "2=theirs", "resolved", false).unwrap(),
            "1o\nmid\n2t\n"
        );
        assert_eq!(
            resolve(text, "keep", "all=theirs", "resolved", false).unwrap(),
            "1t\nmid\n2t\n"
        );
        // Later entries win over earlier overlapping ones.
        assert_eq!(
            resolve(text, "ours", "1-2=theirs, 2=both", "resolved", false).unwrap(),
            "1t\nmid\n2o\n2t\n"
        );
    }

    #[test]
    fn list_reports_numbers_line_spans_labels_and_the_plan() {
        let out = resolve(TWO_WAY, "ours", "1=keep", "list", false).unwrap();
        assert_eq!(
            out,
            "1 conflict · 0 resolved, 1 kept\n\n\
[1] lines 2-6 · ours \"HEAD\" 1 line · theirs \"feature/local\" 1 line → keep"
        );
    }

    #[test]
    fn sides_renders_aligned_columns() {
        let out = resolve(TWO_WAY, "ours", "", "sides", false).unwrap();
        assert_eq!(
            out,
            "1 conflict · 1 resolved\n\n\
[1] lines 2-6 → ours\n\
ours \"HEAD\"             | theirs \"feature/local\"\n\
------------------------+--------------------------\n\
const host = \"0.0.0.0\"; | const host = \"127.0.0.1\";"
        );
    }

    #[test]
    fn json_carries_the_inventory_and_the_resolved_text() {
        let out = resolve(DIFF3, "theirs", "", "json", false).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["conflict_count"], 1);
        assert_eq!(v["resolved_count"], 1);
        assert_eq!(v["conflict_style"], "diff3");
        assert_eq!(v["line_ending"], "lf");
        assert_eq!(v["conflicts"][0]["base"][0], "a-base");
        assert_eq!(v["conflicts"][0]["theirs_label"], "topic");
        assert_eq!(v["resolved"], "a-theirs\n");
    }

    #[test]
    fn text_without_conflicts_passes_through_unchanged() {
        let plain = "# Title\n=======\nstill markdown\n";
        assert_eq!(resolve(plain, "ours", "", "resolved", false).unwrap(), plain);
        assert_eq!(
            resolve(plain, "ours", "", "list", false).unwrap(),
            "0 conflicts · no conflict markers found"
        );
    }

    #[test]
    fn crlf_and_missing_trailing_newline_are_preserved() {
        let crlf = "a\r\n<<<<<<< HEAD\r\nours\r\n=======\r\ntheirs\r\n>>>>>>> b\r\nz";
        assert_eq!(
            resolve(crlf, "theirs", "", "resolved", false).unwrap(),
            "a\r\ntheirs\r\nz"
        );
    }

    #[test]
    fn strict_rejects_marker_free_input_and_unresolved_blocks() {
        let err = resolve("nothing here\n", "ours", "", "resolved", true).unwrap_err();
        assert!(err.contains("no conflict markers"), "{err}");
        let err = resolve(TWO_WAY, "keep", "", "resolved", true).unwrap_err();
        assert!(err.contains("conflict [1] at line 2 is left unresolved"), "{err}");
        assert!(resolve(TWO_WAY, "ours", "", "resolved", true).is_ok());
    }

    #[test]
    fn malformed_marker_sequences_are_located_errors() {
        let err = resolve("<<<<<<< HEAD\nours\n", "ours", "", "resolved", false).unwrap_err();
        assert_eq!(
            err,
            "unterminated conflict: '<<<<<<<' at line 1 has no matching '>>>>>>>'"
        );
        let err = resolve("<<<<<<<\na\n<<<<<<<\n", "ours", "", "resolved", false).unwrap_err();
        assert!(err.contains("nested '<<<<<<<' at line 3"), "{err}");
        let err = resolve("ok\n>>>>>>> other\n", "ours", "", "resolved", false).unwrap_err();
        assert!(err.contains("stray '>>>>>>>' marker at line 2"), "{err}");
        let err = resolve("<<<<<<<\na\n>>>>>>>\n", "ours", "", "resolved", false).unwrap_err();
        assert!(err.contains("no '=======' separator"), "{err}");
    }

    #[test]
    fn invalid_arguments_explain_what_was_expected() {
        let err = resolve(TWO_WAY, "mine", "", "resolved", false).unwrap_err();
        assert!(err.contains("unknown strategy 'mine'"), "{err}");
        let err = resolve(TWO_WAY, "ours", "", "diff", false).unwrap_err();
        assert!(err.contains("unknown output 'diff'"), "{err}");
        let err = resolve(TWO_WAY, "ours", "2", "resolved", false).unwrap_err();
        assert!(err.contains("expected <conflict>=<strategy>"), "{err}");
        let err = resolve(TWO_WAY, "ours", "4=theirs", "resolved", false).unwrap_err();
        assert!(err.contains("refers to conflict 4, but the input has 1 conflict"), "{err}");
        let err = resolve(TWO_WAY, "ours", "x=theirs", "resolved", false).unwrap_err();
        assert!(err.contains("invalid conflict selector 'x'"), "{err}");
        let err = resolve("", "ours", "", "resolved", false).unwrap_err();
        assert!(err.contains("empty input"), "{err}");
    }

    #[test]
    fn the_one_megabyte_cap_is_enforced() {
        let filler = "x".repeat(MAX_INPUT_BYTES - TWO_WAY.len());
        let at_cap = format!("{TWO_WAY}{filler}");
        assert_eq!(at_cap.len(), MAX_INPUT_BYTES);
        assert!(resolve(&at_cap, "ours", "", "list", false).is_ok());
        let over = format!("{at_cap}x");
        let err = resolve(&over, "ours", "", "list", false).unwrap_err();
        assert!(err.contains("input too large"), "{err}");
    }
}
