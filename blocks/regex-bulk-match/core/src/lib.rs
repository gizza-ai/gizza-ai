//! regex-bulk-match core — pure compute, shared by the chat skill block and the web page.
//!
//! Test ONE regular expression against MANY input lines and report a verdict per
//! line: matched or not matched, plus the matched text, its character offsets, and
//! every capture group (named groups by name, unnamed by index). Reports can be
//! filtered to only matching / only non-matching lines and rendered as a text
//! report, a JSON object, or CSV. Pure-Rust (`regex`); no wafer/wasm-bindgen deps.

use regex::{Regex, RegexBuilder};
use serde::Serialize;

/// Hard ceiling on `max_lines`, independent of what the caller asks for.
pub const MAX_LINES_CAP: usize = 20_000;

/// One capture group on a matched line.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Group {
    /// 1-based capture-group index.
    pub index: usize,
    /// The group's name when the pattern used `(?<name>…)`, else `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The captured text, or `None` when the group did not participate.
    pub value: Option<String>,
}

impl Group {
    /// Display label: the group name when present, else `1`, `2`, …
    pub fn label(&self) -> String {
        match &self.name {
            Some(n) => n.clone(),
            None => self.index.to_string(),
        }
    }
}

/// The verdict for one input line.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LineResult {
    /// 1-based line number in the original input (blank lines still count).
    pub line: usize,
    /// The line as tested (after trimming, when trimming is on).
    pub text: String,
    /// Whether the pattern matched this line.
    pub matched: bool,
    /// The matched substring (the whole match, group 0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#match: Option<String>,
    /// Byte offset of the match start within `text`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<usize>,
    /// Byte offset of the match end within `text`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<usize>,
    /// Capture groups 1..n (empty when the pattern has none or the line did not match).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<Group>,
}

/// The full report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// How many lines were actually tested.
    pub lines_tested: usize,
    /// How many of them matched.
    pub matched: usize,
    /// How many of them did not match.
    pub not_matched: usize,
    /// True when the input had more lines than `max_lines` allowed.
    pub truncated: bool,
    /// The reported lines, after the `show` filter.
    pub results: Vec<LineResult>,
}

/// Which lines to report.
fn parse_show(show: &str) -> Result<&'static str, String> {
    Ok(match show.trim() {
        "" | "all" => "all",
        "matching" => "matching",
        "non-matching" => "non-matching",
        other => {
            return Err(format!(
                "unknown show '{other}' — use all, matching, or non-matching"
            ))
        }
    })
}

/// Which renderer to use.
fn parse_output(output: &str) -> Result<&'static str, String> {
    Ok(match output.trim() {
        "" | "text" => "text",
        "json" => "json",
        "csv" => "csv",
        other => return Err(format!("unknown output '{other}' — use text, json, or csv")),
    })
}

/// Compile the pattern, anchoring it to the whole line when `full_match` is on.
fn compile(
    pattern: &str,
    full_match: bool,
    ignore_case: bool,
    dotall: bool,
) -> Result<Regex, String> {
    let src = if full_match {
        // Wrap rather than concatenating `^`/`$` so alternations anchor as a whole
        // (`a|b` must become `^(?:a|b)$`, not `^a|b$`).
        format!("^(?:{pattern})$")
    } else {
        pattern.to_string()
    };
    RegexBuilder::new(&src)
        .case_insensitive(ignore_case)
        .dot_matches_new_line(dotall)
        .build()
        .map_err(|e| format!("invalid regex pattern: {e}"))
}

/// Collect the capture groups for one match, keeping unmatched optional groups as `None`.
fn groups_for(re: &Regex, caps: &regex::Captures<'_>) -> Vec<Group> {
    let names: Vec<Option<String>> = re
        .capture_names()
        .map(|n| n.map(|s| s.to_string()))
        .collect();
    (1..re.captures_len())
        .map(|i| Group {
            index: i,
            name: names.get(i).cloned().flatten(),
            value: caps.get(i).map(|m| m.as_str().to_string()),
        })
        .collect()
}

/// Escape one CSV field (RFC 4180 minimal quoting).
fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Test `pattern` against each line of `lines`.
///
/// * `full_match` — require the pattern to match the whole line (off = match anywhere).
/// * `ignore_case` — case-insensitive matching.
/// * `dotall` — `.` also matches a newline (only meaningful with multi-line captures).
/// * `trim` — strip leading/trailing whitespace from each line before testing.
/// * `skip_blank` — drop lines that are empty (after trimming) instead of testing them.
/// * `captures` — include capture groups in the report.
/// * `show_position` — include the match offsets in the text and CSV reports.
/// * `show` — `all` | `matching` | `non-matching`.
/// * `max_lines` — stop after this many tested lines (capped at [`MAX_LINES_CAP`]).
/// * `output` — `text` | `json` | `csv`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    lines: &str,
    pattern: &str,
    full_match: bool,
    ignore_case: bool,
    dotall: bool,
    trim: bool,
    skip_blank: bool,
    captures: bool,
    show_position: bool,
    show: &str,
    max_lines: usize,
    output: &str,
) -> Result<String, String> {
    if lines.trim().is_empty() {
        return Err("input lines are required".into());
    }
    if pattern.is_empty() {
        return Err("pattern is required".into());
    }
    let show = parse_show(show)?;
    let output = parse_output(output)?;
    if max_lines == 0 {
        return Err("max_lines must be at least 1".into());
    }
    let limit = max_lines.min(MAX_LINES_CAP);

    let re = compile(pattern, full_match, ignore_case, dotall)?;

    let mut tested = 0usize;
    let mut matched = 0usize;
    let mut truncated = false;
    let mut results: Vec<LineResult> = Vec::new();

    for (idx, raw) in lines.lines().enumerate() {
        let text = if trim { raw.trim() } else { raw };
        if skip_blank && text.trim().is_empty() {
            continue;
        }
        if tested >= limit {
            truncated = true;
            break;
        }
        tested += 1;

        let caps = re.captures(text);
        let is_match = caps.is_some();
        if is_match {
            matched += 1;
        }
        let keep = match show {
            "matching" => is_match,
            "non-matching" => !is_match,
            _ => true,
        };
        if !keep {
            continue;
        }
        let (m, start, end, groups) = match caps {
            Some(c) => {
                let whole = c.get(0).expect("group 0 always participates");
                let g = if captures {
                    groups_for(&re, &c)
                } else {
                    Vec::new()
                };
                (
                    Some(whole.as_str().to_string()),
                    Some(whole.start()),
                    Some(whole.end()),
                    g,
                )
            }
            None => (None, None, None, Vec::new()),
        };
        results.push(LineResult {
            line: idx + 1,
            text: text.to_string(),
            matched: is_match,
            r#match: m,
            start,
            end,
            groups,
        });
    }

    if tested == 0 {
        return Err("no lines to test — every input line was blank".into());
    }

    let report = Report {
        lines_tested: tested,
        matched,
        not_matched: tested - matched,
        truncated,
        results,
    };

    Ok(match output {
        "json" => serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
        "csv" => render_csv(&report, &re, captures, show_position),
        _ => render_text(&report, captures, show_position),
    })
}

/// Human-readable per-line report.
fn render_text(report: &Report, captures: bool, show_position: bool) -> String {
    let mut out = String::new();
    out.push_str(&format!("Lines tested: {}\n", report.lines_tested));
    out.push_str(&format!("Matched: {}\n", report.matched));
    out.push_str(&format!("Not matched: {}\n", report.not_matched));
    if report.truncated {
        out.push_str("Truncated: input had more lines than the max-lines limit\n");
    }
    out.push('\n');
    if report.results.is_empty() {
        out.push_str("No lines to show for the selected filter.\n");
        return out;
    }
    for r in &report.results {
        let verdict = if r.matched { "MATCH   " } else { "NO MATCH" };
        out.push_str(&format!("line {}: {} {:?}", r.line, verdict, r.text));
        if show_position {
            if let (Some(s), Some(e)) = (r.start, r.end) {
                out.push_str(&format!(" at {s}..{e}"));
            }
        }
        if captures && !r.groups.is_empty() {
            let parts: Vec<String> = r
                .groups
                .iter()
                .map(|g| format!("{}={}", g.label(), g.value.clone().unwrap_or_default()))
                .collect();
            out.push_str(&format!(" | {}", parts.join(", ")));
        }
        out.push('\n');
    }
    out
}

/// One CSV row per reported line; one column per capture group.
fn render_csv(report: &Report, re: &Regex, captures: bool, show_position: bool) -> String {
    let names: Vec<Option<String>> = re
        .capture_names()
        .map(|n| n.map(|s| s.to_string()))
        .collect();
    let group_count = if captures { re.captures_len() - 1 } else { 0 };

    let mut header = vec![
        "line".to_string(),
        "text".to_string(),
        "matched".to_string(),
    ];
    header.push("match".to_string());
    if show_position {
        header.push("start".to_string());
        header.push("end".to_string());
    }
    for i in 1..=group_count {
        header.push(match names.get(i).cloned().flatten() {
            Some(n) => n,
            None => format!("group_{i}"),
        });
    }

    let mut out = String::new();
    out.push_str(
        &header
            .iter()
            .map(|h| csv_field(h))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push('\n');
    for r in &report.results {
        let mut row = vec![
            r.line.to_string(),
            r.text.clone(),
            r.matched.to_string(),
            r.r#match.clone().unwrap_or_default(),
        ];
        if show_position {
            row.push(r.start.map(|v| v.to_string()).unwrap_or_default());
            row.push(r.end.map(|v| v.to_string()).unwrap_or_default());
        }
        for i in 1..=group_count {
            let v = r
                .groups
                .iter()
                .find(|g| g.index == i)
                .and_then(|g| g.value.clone())
                .unwrap_or_default();
            row.push(v);
        }
        out.push_str(
            &row.iter()
                .map(|c| csv_field(c))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMAILS: &str = "ada@example.com\nnot-an-email\nbo@test.org";

    fn json(out: &str) -> serde_json::Value {
        serde_json::from_str(out).unwrap()
    }

    // --- happy paths ---

    #[test]
    fn reports_match_and_no_match_per_line() {
        let out = run(
            EMAILS,
            r"^[\w.+-]+@([\w-]+\.[\w.]+)$",
            false,
            false,
            false,
            true,
            true,
            true,
            false,
            "all",
            1000,
            "json",
        )
        .unwrap();
        let v = json(&out);
        assert_eq!(v["lines_tested"], 3);
        assert_eq!(v["matched"], 2);
        assert_eq!(v["not_matched"], 1);
        assert_eq!(v["results"][0]["matched"], true);
        assert_eq!(v["results"][0]["groups"][0]["value"], "example.com");
        assert_eq!(v["results"][1]["matched"], false);
        assert_eq!(v["results"][2]["groups"][0]["value"], "test.org");
    }

    #[test]
    fn text_report_is_exact() {
        let out = run(
            "abc123\nxyz",
            r"([a-z]+)(\d+)",
            false,
            false,
            false,
            true,
            true,
            true,
            false,
            "all",
            1000,
            "text",
        )
        .unwrap();
        assert_eq!(
            out,
            "Lines tested: 2\nMatched: 1\nNot matched: 1\n\n\
             line 1: MATCH    \"abc123\" | 1=abc, 2=123\n\
             line 2: NO MATCH \"xyz\"\n"
        );
    }

    #[test]
    fn named_groups_use_their_names() {
        let out = run(
            "2026-08-15",
            r"(?<y>\d{4})-(?<m>\d{2})-(?<d>\d{2})",
            true,
            false,
            false,
            true,
            true,
            true,
            false,
            "all",
            1000,
            "text",
        )
        .unwrap();
        assert!(out.contains("y=2026, m=08, d=15"), "{out}");
    }

    #[test]
    fn full_match_anchors_the_whole_line() {
        // "anywhere" mode matches the digits inside the longer string...
        let anywhere = run(
            "x123x", r"\d+", false, false, false, true, true, true, false, "all", 1000, "json",
        )
        .unwrap();
        assert_eq!(json(&anywhere)["matched"], 1);
        // ...but whole-line mode does not.
        let whole = run(
            "x123x", r"\d+", true, false, false, true, true, true, false, "all", 1000, "json",
        )
        .unwrap();
        assert_eq!(json(&whole)["matched"], 0);
    }

    #[test]
    fn full_match_anchors_alternations_as_a_group() {
        let out = run(
            "yes\nno\nmaybe so",
            "yes|no",
            true,
            false,
            false,
            true,
            true,
            true,
            false,
            "all",
            1000,
            "json",
        )
        .unwrap();
        assert_eq!(json(&out)["matched"], 2);
    }

    #[test]
    fn ignore_case_flag_works() {
        let out = run(
            "HELLO", "hello", true, true, false, true, true, true, false, "all", 1000, "json",
        )
        .unwrap();
        assert_eq!(json(&out)["matched"], 1);
    }

    #[test]
    fn inline_flags_work() {
        let out = run(
            "HELLO",
            "(?i)hello",
            true,
            false,
            false,
            true,
            true,
            true,
            false,
            "all",
            1000,
            "json",
        )
        .unwrap();
        assert_eq!(json(&out)["matched"], 1);
    }

    #[test]
    fn show_filter_limits_reported_lines() {
        let only_bad = run(
            EMAILS,
            r"@",
            false,
            false,
            false,
            true,
            true,
            true,
            false,
            "non-matching",
            1000,
            "json",
        )
        .unwrap();
        let v = json(&only_bad);
        assert_eq!(v["lines_tested"], 3);
        assert_eq!(v["results"].as_array().unwrap().len(), 1);
        assert_eq!(v["results"][0]["text"], "not-an-email");

        let only_good = run(
            EMAILS, r"@", false, false, false, true, true, true, false, "matching", 1000, "json",
        )
        .unwrap();
        assert_eq!(json(&only_good)["results"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn positions_are_reported_when_asked() {
        let out = run(
            "xx42", r"\d+", false, false, false, true, true, true, true, "all", 1000, "text",
        )
        .unwrap();
        assert!(out.contains("at 2..4"), "{out}");
        // JSON always carries the offsets.
        let j = run(
            "xx42", r"\d+", false, false, false, true, true, true, false, "all", 1000, "json",
        )
        .unwrap();
        assert_eq!(json(&j)["results"][0]["start"], 2);
        assert_eq!(json(&j)["results"][0]["end"], 4);
    }

    #[test]
    fn captures_can_be_switched_off() {
        let out = run(
            "abc123",
            r"([a-z]+)(\d+)",
            false,
            false,
            false,
            true,
            true,
            false,
            false,
            "all",
            1000,
            "json",
        )
        .unwrap();
        assert!(json(&out)["results"][0].get("groups").is_none());
    }

    #[test]
    fn csv_output_has_a_column_per_group() {
        let out = run(
            "ada@example.com\nnope",
            r"([\w.]+)@([\w.]+)",
            false,
            false,
            false,
            true,
            true,
            true,
            false,
            "all",
            1000,
            "csv",
        )
        .unwrap();
        assert_eq!(
            out,
            "line,text,matched,match,group_1,group_2\n\
             1,ada@example.com,true,ada@example.com,ada,example.com\n\
             2,nope,false,,,\n"
        );
    }

    #[test]
    fn csv_quotes_fields_with_commas() {
        let out = run(
            "a,b", r"a,b", true, false, false, true, true, true, false, "all", 1000, "csv",
        )
        .unwrap();
        assert!(out.contains("\"a,b\",true"), "{out}");
    }

    #[test]
    fn blank_lines_are_skipped_by_default() {
        let out = run(
            "a\n\n\nb", r"\w", false, false, false, true, true, true, false, "all", 1000, "json",
        )
        .unwrap();
        assert_eq!(json(&out)["lines_tested"], 2);
    }

    #[test]
    fn blank_lines_can_be_tested() {
        let out = run(
            "a\n\nb", r"\w", false, false, false, true, false, true, false, "all", 1000, "json",
        )
        .unwrap();
        let v = json(&out);
        assert_eq!(v["lines_tested"], 3);
        assert_eq!(v["not_matched"], 1);
        // Line numbers still refer to the original input.
        assert_eq!(v["results"][2]["line"], 3);
    }

    #[test]
    fn trimming_is_on_by_default_and_can_be_turned_off() {
        let trimmed = run(
            "  42  ", r"^\d+$", false, false, false, true, true, true, false, "all", 1000, "json",
        )
        .unwrap();
        assert_eq!(json(&trimmed)["matched"], 1);
        let untrimmed = run(
            "  42  ", r"^\d+$", false, false, false, false, true, true, false, "all", 1000, "json",
        )
        .unwrap();
        assert_eq!(json(&untrimmed)["matched"], 0);
    }

    #[test]
    fn dotall_only_affects_dot() {
        let out = run(
            "a.b", r"a.b", true, false, true, true, true, true, false, "all", 1000, "json",
        )
        .unwrap();
        assert_eq!(json(&out)["matched"], 1);
    }

    #[test]
    fn max_lines_truncates_and_flags_it() {
        let out = run(
            "a\nb\nc\nd",
            r"\w",
            false,
            false,
            false,
            true,
            true,
            true,
            false,
            "all",
            2,
            "json",
        )
        .unwrap();
        let v = json(&out);
        assert_eq!(v["lines_tested"], 2);
        assert_eq!(v["truncated"], true);
    }

    #[test]
    fn at_the_cap_boundary_nothing_is_truncated() {
        let out = run(
            "a\nb\nc", r"\w", false, false, false, true, true, true, false, "all", 3, "json",
        )
        .unwrap();
        let v = json(&out);
        assert_eq!(v["lines_tested"], 3);
        assert_eq!(v["truncated"], false);
    }

    #[test]
    fn optional_group_that_did_not_participate_is_blank() {
        let out = run(
            "ab", r"a(x)?b", true, false, false, true, true, true, false, "all", 1000, "csv",
        )
        .unwrap();
        assert_eq!(out, "line,text,matched,match,group_1\n1,ab,true,ab,\n");
    }

    // --- error paths ---

    #[test]
    fn empty_input_errors() {
        let err = run(
            "", r"\d", false, false, false, true, true, true, false, "all", 1000, "text",
        )
        .unwrap_err();
        assert!(err.contains("input lines are required"), "{err}");
    }

    #[test]
    fn empty_pattern_errors() {
        let err = run(
            "abc", "", false, false, false, true, true, true, false, "all", 1000, "text",
        )
        .unwrap_err();
        assert!(err.contains("pattern is required"), "{err}");
    }

    #[test]
    fn invalid_regex_errors() {
        let err = run(
            "abc",
            "[unclosed",
            false,
            false,
            false,
            true,
            true,
            true,
            false,
            "all",
            1000,
            "text",
        )
        .unwrap_err();
        assert!(err.contains("invalid regex pattern"), "{err}");
    }

    #[test]
    fn unknown_show_errors() {
        let err = run(
            "abc", r"\w", false, false, false, true, true, true, false, "sideways", 1000, "text",
        )
        .unwrap_err();
        assert!(err.contains("unknown show"), "{err}");
    }

    #[test]
    fn unknown_output_errors() {
        let err = run(
            "abc", r"\w", false, false, false, true, true, true, false, "all", 1000, "xml",
        )
        .unwrap_err();
        assert!(err.contains("unknown output"), "{err}");
    }

    #[test]
    fn zero_max_lines_errors() {
        let err = run(
            "abc", r"\w", false, false, false, true, true, true, false, "all", 0, "text",
        )
        .unwrap_err();
        assert!(err.contains("max_lines must be at least 1"), "{err}");
    }

    #[test]
    fn all_blank_input_errors() {
        let err = run(
            "   \n  \n",
            r"\w",
            false,
            false,
            false,
            true,
            true,
            true,
            false,
            "all",
            1000,
            "text",
        )
        .unwrap_err();
        assert!(err.contains("input lines are required"), "{err}");
    }

    #[test]
    fn max_lines_above_the_hard_cap_is_clamped_not_rejected() {
        let out = run(
            "a\nb",
            r"\w",
            false,
            false,
            false,
            true,
            true,
            true,
            false,
            "all",
            MAX_LINES_CAP * 10,
            "json",
        )
        .unwrap();
        let v = json(&out);
        assert_eq!(v["lines_tested"], 2);
        assert_eq!(v["truncated"], false);
    }
}
