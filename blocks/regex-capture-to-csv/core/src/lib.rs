//! gizza-ai/regex-capture-to-csv core — scan text with a regular expression and
//! emit one RFC-4180 CSV row per match, with the capture groups as columns.
//! Pure-Rust (`regex` only). No wafer/wasm-bindgen deps; shared by the chat
//! skill block, the CLI, and the web page.
//!
//! Distinct from the neighbouring regex tools:
//! - `regex-extract` returns a flat list of ONE group's matches;
//! - `regex-tester` shows a per-match span/group debugging breakdown;
//! - `regex-to-json` is LINE-oriented (one record per line) and emits JSON.
//!
//! This tool scans the WHOLE text — a match may span several lines — and emits
//! spreadsheet-ready CSV: a header row of group names plus one row per match,
//! with real CSV quoting, a chosen delimiter, column selection/reordering,
//! dedupe, and sort.

use regex::RegexBuilder;
use std::collections::HashSet;

/// Maximum accepted input size in bytes (1 MB). The regex engine is
/// linear-time, but the browser page and the chat sandbox share small memory
/// budgets — reject anything larger with an actionable error.
pub const MAX_TEXT_BYTES: usize = 1_000_000;

/// Maximum number of emitted rows. A pattern that can match the empty string
/// matches at every position, so 1 MB of input could otherwise produce a
/// million-row CSV.
pub const MAX_ROWS: usize = 100_000;

/// How aggressively to quote fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quoting {
    /// Quote only when the value contains the delimiter, a quote, CR or LF.
    Minimal,
    /// Quote every field, including the header row.
    All,
}

impl Quoting {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "minimal" => Ok(Self::Minimal),
            "all" => Ok(Self::All),
            other => Err(format!(
                "unknown quoting mode '{other}' — use minimal or all"
            )),
        }
    }
}

/// Row terminator for the rendered CSV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineEnding {
    Lf,
    Crlf,
}

impl LineEnding {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "lf" => Ok(Self::Lf),
            "crlf" => Ok(Self::Crlf),
            other => Err(format!(
                "unknown line ending '{other}' — use lf or crlf"
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
        }
    }
}

/// Resolve a delimiter spec (a single character, a `\t` escape, or a keyword)
/// into the character used between fields.
fn parse_delimiter(spec: &str) -> Result<char, String> {
    let d = match spec {
        "" | "," | "comma" => ',',
        "tab" | "\t" | "\\t" => '\t',
        ";" | "semicolon" => ';',
        "|" | "pipe" => '|',
        ":" | "colon" => ':',
        " " | "space" => ' ',
        other => {
            let mut chars = other.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => c,
                _ => {
                    return Err(format!(
                    "unknown delimiter '{other}' — use a single character, or one of comma, \
                     semicolon, tab, pipe, colon, space"
                ))
                }
            }
        }
    };
    if matches!(d, '"' | '\n' | '\r') {
        return Err("delimiter cannot be a quote or a line break".to_string());
    }
    Ok(d)
}

/// Escape one field for CSV output.
fn render_field(value: &str, delimiter: char, quoting: Quoting) -> String {
    let must_quote = quoting == Quoting::All
        || value.contains(delimiter)
        || value.contains('"')
        || value.contains('\n')
        || value.contains('\r');
    if must_quote {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Scan `text` with `pattern` and render one CSV row per match.
///
/// Columns come from the pattern's capture groups, in pattern order:
/// - NAMED groups (`(?<name>…)` / `(?P<name>…)`) if the pattern has any — their
///   names become the header, and unnamed groups are ignored;
/// - otherwise numbered groups, headed `column1`, `column2`, …;
/// - otherwise (no groups at all) a single `match` column holding the whole match.
///
/// - `columns`: comma-separated subset of those names, in the order you want
///   them (blank = every column in pattern order). Names may repeat.
/// - `delimiter`: a single character, `\t`, or a keyword (comma, semicolon,
///   tab, pipe, colon, space).
/// - `header`: emit the header row of column names.
/// - `quoting`: `minimal` (quote only when needed) or `all` (quote every field).
/// - `line_ending`: `lf` or `crlf` (Excel-friendly).
/// - `ignore_case` / `multiline` / `dotall`: the regex `i` / `m` / `s` flags.
///   With `dotall`, `.` matches newlines so a single match can span lines.
/// - `unique`: drop duplicate rows, keeping first-seen order.
/// - `sort`: sort rows lexicographically (applied after `unique`).
///
/// A group that did not participate in a match yields an empty field, so every
/// row has the same number of columns.
#[allow(clippy::too_many_arguments)]
pub fn to_csv(
    text: &str,
    pattern: &str,
    columns: &str,
    delimiter: &str,
    header: bool,
    quoting: &str,
    line_ending: &str,
    ignore_case: bool,
    multiline: bool,
    dotall: bool,
    unique: bool,
    sort: bool,
) -> Result<String, String> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(format!(
            "input is {} bytes — the limit is {} bytes (1 MB)",
            text.len(),
            MAX_TEXT_BYTES
        ));
    }
    if pattern.trim().is_empty() {
        return Err("pattern is required — enter a regular expression".to_string());
    }

    let delim = parse_delimiter(delimiter)?;
    let quoting = Quoting::parse(quoting)?;
    let eol = LineEnding::parse(line_ending)?;

    let re = RegexBuilder::new(pattern)
        .case_insensitive(ignore_case)
        .multi_line(multiline)
        .dot_matches_new_line(dotall)
        .build()
        .map_err(|e| format!("invalid regular expression: {e}"))?;

    // Column name → capture index, in pattern order.
    let named: Vec<(String, usize)> = re
        .capture_names()
        .enumerate()
        .skip(1)
        .filter_map(|(i, n)| n.map(|n| (n.to_string(), i)))
        .collect();
    let available: Vec<(String, usize)> = if !named.is_empty() {
        named
    } else if re.captures_len() > 1 {
        (1..re.captures_len())
            .map(|i| (format!("column{i}"), i))
            .collect()
    } else {
        vec![("match".to_string(), 0)]
    };

    let selected: Vec<(String, usize)> = if columns.trim().is_empty() {
        available.clone()
    } else {
        let mut picked = Vec::new();
        for want in columns.split(',') {
            let want = want.trim();
            if want.is_empty() {
                continue;
            }
            match available.iter().find(|(name, _)| name == want) {
                Some(col) => picked.push(col.clone()),
                None => {
                    let names: Vec<&str> = available.iter().map(|(n, _)| n.as_str()).collect();
                    return Err(format!(
                        "unknown column '{want}' — this pattern provides: {}",
                        names.join(", ")
                    ));
                }
            }
        }
        if picked.is_empty() {
            return Err("columns lists no usable column names".to_string());
        }
        picked
    };

    let mut rows: Vec<Vec<String>> = Vec::new();
    for caps in re.captures_iter(text) {
        if rows.len() >= MAX_ROWS {
            return Err(format!(
                "the pattern produced more than {MAX_ROWS} rows — narrow the pattern or shorten the text"
            ));
        }
        rows.push(
            selected
                .iter()
                .map(|(_, idx)| {
                    caps.get(*idx)
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default()
                })
                .collect(),
        );
    }

    if rows.is_empty() {
        return Err("no matches — the pattern did not match anywhere in the text".to_string());
    }

    if unique {
        let mut seen: HashSet<Vec<String>> = HashSet::new();
        rows.retain(|row| seen.insert(row.clone()));
    }
    if sort {
        rows.sort();
    }

    let mut out: Vec<String> = Vec::with_capacity(rows.len() + 1);
    if header {
        out.push(
            selected
                .iter()
                .map(|(name, _)| render_field(name, delim, quoting))
                .collect::<Vec<_>>()
                .join(&delim.to_string()),
        );
    }
    for row in &rows {
        out.push(
            row.iter()
                .map(|v| render_field(v, delim, quoting))
                .collect::<Vec<_>>()
                .join(&delim.to_string()),
        );
    }
    Ok(out.join(eol.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple(text: &str, pattern: &str) -> Result<String, String> {
        to_csv(
            text, pattern, "", ",", true, "minimal", "lf", false, false, false, false, false,
        )
    }

    #[test]
    fn named_groups_become_columns() {
        let out = simple(
            "alice 30\nbob 41\n",
            r"(?<name>[a-z]+) (?<age>\d+)",
        )
        .unwrap();
        assert_eq!(out, "name,age\nalice,30\nbob,41");
    }

    #[test]
    fn quotes_and_escapes_fields_that_need_it() {
        let out = simple(r#"say "hi", now"#, r#"(?<phrase>say "hi", now)"#).unwrap();
        assert_eq!(out, "phrase\n\"say \"\"hi\"\", now\"");
    }

    #[test]
    fn unnamed_groups_fall_back_to_numbered_columns() {
        let out = simple("a=1 b=2", r"(\w)=(\d)").unwrap();
        assert_eq!(out, "column1,column2\na,1\nb,2");
    }

    #[test]
    fn no_groups_emits_whole_match_column() {
        let out = simple("x1 y2", r"\w\d").unwrap();
        assert_eq!(out, "match\nx1\ny2");
    }

    #[test]
    fn selects_and_reorders_columns() {
        let out = to_csv(
            "alice 30",
            r"(?<name>[a-z]+) (?<age>\d+)",
            "age, name",
            ",",
            true,
            "minimal",
            "lf",
            false,
            false,
            false,
            false,
            false,
        )
        .unwrap();
        assert_eq!(out, "age,name\n30,alice");
    }

    #[test]
    fn tab_delimiter_header_off_and_crlf() {
        let out = to_csv(
            "alice 30\nbob 41",
            r"(?<name>[a-z]+) (?<age>\d+)",
            "",
            "tab",
            false,
            "minimal",
            "crlf",
            false,
            false,
            false,
            false,
            false,
        )
        .unwrap();
        assert_eq!(out, "alice\t30\r\nbob\t41");
    }

    #[test]
    fn quote_all_quotes_every_field_including_header() {
        let out = to_csv(
            "alice 30",
            r"(?<name>[a-z]+) (?<age>\d+)",
            "",
            ",",
            true,
            "all",
            "lf",
            false,
            false,
            false,
            false,
            false,
        )
        .unwrap();
        assert_eq!(out, "\"name\",\"age\"\n\"alice\",\"30\"");
    }

    #[test]
    fn unique_and_sort_apply_in_order() {
        let out = to_csv(
            "b 2\na 1\nb 2",
            r"(?<k>[a-z]) (?<v>\d)",
            "",
            ",",
            false,
            "minimal",
            "lf",
            false,
            false,
            false,
            true,
            true,
        )
        .unwrap();
        assert_eq!(out, "a,1\nb,2");
    }

    #[test]
    fn ignore_case_flag_matches_uppercase() {
        let out = to_csv(
            "ALICE 30",
            r"(?<name>[a-z]+) (?<age>\d+)",
            "",
            ",",
            true,
            "minimal",
            "lf",
            true,
            false,
            false,
            false,
            false,
        )
        .unwrap();
        assert_eq!(out, "name,age\nALICE,30");
    }

    #[test]
    fn dotall_lets_a_match_span_lines() {
        let out = to_csv(
            "<td>one\ntwo</td>",
            r"<td>(?<cell>.+?)</td>",
            "",
            ",",
            false,
            "minimal",
            "lf",
            false,
            false,
            true,
            false,
            false,
        )
        .unwrap();
        assert_eq!(out, "\"one\ntwo\"");
    }

    #[test]
    fn multiline_flag_anchors_per_line() {
        let out = to_csv(
            "one\ntwo",
            r"^(?<word>\w+)$",
            "",
            ",",
            false,
            "minimal",
            "lf",
            false,
            true,
            false,
            false,
            false,
        )
        .unwrap();
        assert_eq!(out, "one\ntwo");
    }

    #[test]
    fn optional_group_yields_an_empty_field() {
        let out = simple("a1 b", r"(?<letter>[a-z])(?<digit>\d)?").unwrap();
        assert_eq!(out, "letter,digit\na,1\nb,");
    }

    #[test]
    fn invalid_regex_is_an_error() {
        let err = simple("x", "(?<oops>").unwrap_err();
        assert!(err.starts_with("invalid regular expression:"), "{err}");
    }

    #[test]
    fn empty_pattern_is_an_error() {
        let err = simple("x", "  ").unwrap_err();
        assert!(err.contains("pattern is required"), "{err}");
    }

    #[test]
    fn no_match_is_an_error() {
        let err = simple("abc", r"(?<n>\d+)").unwrap_err();
        assert!(err.contains("no matches"), "{err}");
    }

    #[test]
    fn unknown_column_lists_available_names() {
        let err = to_csv(
            "alice 30",
            r"(?<name>[a-z]+) (?<age>\d+)",
            "nope",
            ",",
            true,
            "minimal",
            "lf",
            false,
            false,
            false,
            false,
            false,
        )
        .unwrap_err();
        assert!(err.contains("unknown column 'nope'"), "{err}");
        assert!(err.contains("name, age"), "{err}");
    }

    #[test]
    fn bad_delimiter_and_modes_are_errors() {
        let text = "alice 30";
        let pat = r"(?<name>[a-z]+) (?<age>\d+)";
        let err = to_csv(
            text, pat, "", "double-pipe", true, "minimal", "lf", false, false, false, false, false,
        )
        .unwrap_err();
        assert!(err.contains("unknown delimiter"), "{err}");
        let err = to_csv(
            text, pat, "", ",", true, "loose", "lf", false, false, false, false, false,
        )
        .unwrap_err();
        assert!(err.contains("unknown quoting mode"), "{err}");
        let err = to_csv(
            text, pat, "", ",", true, "minimal", "cr", false, false, false, false, false,
        )
        .unwrap_err();
        assert!(err.contains("unknown line ending"), "{err}");
    }

    #[test]
    fn oversize_input_is_rejected_at_the_cap() {
        let big = "a".repeat(MAX_TEXT_BYTES + 1);
        let err = simple(&big, r"(?<c>a)").unwrap_err();
        assert!(err.contains("the limit is 1000000 bytes"), "{err}");
        // Exactly at the cap still runs: one greedy match over the whole run.
        let at_cap = "a".repeat(MAX_TEXT_BYTES);
        let ok = simple(&at_cap, r"(?<c>a+)").unwrap();
        assert_eq!(ok.len(), "c\n".len() + MAX_TEXT_BYTES);
        assert!(ok.starts_with("c\na"), "{}", &ok[..8.min(ok.len())]);
    }

    #[test]
    fn too_many_rows_is_an_error() {
        let text = "a".repeat(MAX_ROWS + 1);
        let err = simple(&text, r"(?<c>a)").unwrap_err();
        assert!(err.contains("more than 100000 rows"), "{err}");
    }
}
