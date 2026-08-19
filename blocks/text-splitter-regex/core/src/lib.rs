//! gizza-ai/text-splitter-regex core — split text into rows (and optionally
//! fields) using a regular expression as the delimiter. Pure-Rust (`regex`
//! only). No wafer/wasm-bindgen deps; shared by the chat skill block, the CLI,
//! and the web page.
//!
//! Distinct from the neighbouring text/regex tools:
//! - `split-text` splits on a LITERAL substring (or whitespace/chars) — no regex;
//! - `regex-extract` returns the MATCHES of a pattern, the inverse of splitting on it;
//! - `regex-capture-to-csv` emits one row per match using capture groups as columns;
//! - `chunk-text` chunks by size/overlap for RAG rather than by a delimiter.
//!
//! Here the pattern is the SEPARATOR: everything between matches becomes a part.
//! A second `field_pattern` splits each row again, turning delimited text into a
//! real two-dimensional table.

use regex::RegexBuilder;

/// Maximum accepted input size in characters (200,000). The regex engine is
/// linear-time, but the browser page and the chat sandbox share small memory
/// budgets — reject anything larger with an actionable error.
pub const MAX_TEXT_CHARS: usize = 200_000;

/// Maximum number of emitted parts (rows × fields). A pattern that can match the
/// empty string matches at every position, so a large input could otherwise
/// produce a part per character.
pub const MAX_PARTS: usize = 100_000;

/// How the resulting parts are rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Output {
    /// One part per line (fields, when present, joined by a tab).
    Lines,
    /// A JSON array of strings — or of arrays of strings when fields are split.
    Json,
    /// RFC-4180 CSV: one row per line, fields separated by commas.
    Csv,
    /// Tab-separated values: one row per line, fields separated by tabs.
    Tsv,
    /// `1. part` per line, numbered from 1.
    Numbered,
    /// Parts joined by the caller's `separator` string.
    Separator,
}

impl Output {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "lines" => Ok(Self::Lines),
            "json" => Ok(Self::Json),
            "csv" => Ok(Self::Csv),
            "tsv" => Ok(Self::Tsv),
            "numbered" => Ok(Self::Numbered),
            "separator" => Ok(Self::Separator),
            other => Err(format!(
                "unknown output format '{other}' — use lines, json, csv, tsv, numbered or separator"
            )),
        }
    }
}

/// Split `text` on the regular expression `pattern`.
///
/// - `pattern`: the SEPARATOR regex (Rust regex syntax), e.g. `\s+`, `[,;|]`,
///   `\n{2,}`. Required — an empty pattern is an error (splitting on "" is
///   meaningless; use the `split-text` tool's `chars` mode for that).
/// - `field_pattern`: an optional second regex that splits every row into
///   fields, producing a table. Blank means rows only.
/// - `ignore_case` / `multiline` / `dotall`: the `i`, `m` and `s` regex flags,
///   applied to both patterns.
/// - `trim`: trim leading/trailing whitespace from every part.
/// - `remove_empty`: drop parts that are empty (after trimming, if `trim` is on).
///   With fields split, a row is dropped only when all of its fields are empty.
/// - `max_splits`: stop after this many splits of the input into rows
///   (0 = unlimited); the remainder is kept intact as the final row, matching
///   `str::splitn`. Field splitting is never capped.
/// - `output`: `lines` | `json` | `csv` | `tsv` | `numbered` | `separator`.
/// - `separator`: the join string for `output = "separator"`. The escapes `\n`,
///   `\t`, `\r` and `\\` are recognised.
///
/// Returns the rendered parts. Returns `Err` on an invalid pattern or output
/// format, on empty input or an empty pattern, or when a limit is exceeded.
#[allow(clippy::too_many_arguments)]
pub fn split(
    text: &str,
    pattern: &str,
    field_pattern: &str,
    ignore_case: bool,
    multiline: bool,
    dotall: bool,
    trim: bool,
    remove_empty: bool,
    max_splits: usize,
    output: &str,
    separator: &str,
) -> Result<String, String> {
    let output = Output::parse(output)?;

    if text.is_empty() {
        return Err("text is empty: paste the text you want to split".to_string());
    }
    let char_count = text.chars().count();
    if char_count > MAX_TEXT_CHARS {
        return Err(format!(
            "text is {char_count} characters, over the {MAX_TEXT_CHARS} character limit — split it into smaller pieces first"
        ));
    }
    if pattern.is_empty() {
        return Err(
            "pattern is empty: give a regular expression to split on, e.g. \\s+ for whitespace runs or [,;|] for several delimiters"
                .to_string(),
        );
    }

    let row_re = build_regex(pattern, "pattern", ignore_case, multiline, dotall)?;
    let field_re = if field_pattern.is_empty() {
        None
    } else {
        Some(build_regex(
            field_pattern,
            "field_pattern",
            ignore_case,
            multiline,
            dotall,
        )?)
    };

    // Split into rows. `max_splits` caps the number of CUTS, so the limit is the
    // part count minus one — `splitn(n)` yields at most n parts.
    let raw_rows: Vec<&str> = if max_splits == 0 {
        row_re.split(text).collect()
    } else {
        row_re.splitn(text, max_splits + 1).collect()
    };

    // Build the table: one Vec<String> per row (a single element when fields are
    // not split), applying trim/remove_empty as we go.
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(raw_rows.len());
    let mut parts = 0usize;
    for raw in raw_rows {
        // `max_splits` caps the ROW split only — field splitting is always
        // complete, so a capped run still yields whole rows.
        let mut fields: Vec<String> = match &field_re {
            None => vec![raw.to_string()],
            Some(re) => re.split(raw).map(str::to_string).collect(),
        };
        if trim {
            for f in &mut fields {
                *f = f.trim().to_string();
            }
        }
        if remove_empty {
            fields.retain(|f| !f.is_empty());
            // A row that lost every field carries no data — drop it too.
            if fields.is_empty() {
                continue;
            }
        }
        parts += fields.len();
        if parts > MAX_PARTS {
            return Err(format!(
                "the pattern produced more than {MAX_PARTS} parts — use a more specific pattern, or set max_splits to cap the split count"
            ));
        }
        rows.push(fields);
    }

    if rows.is_empty() {
        return Err(
            "no parts left after splitting — every part was empty and remove_empty is on"
                .to_string(),
        );
    }

    Ok(render(output, &rows, separator, field_re.is_some()))
}

/// Compile one of the two patterns with the shared flags, naming the offending
/// field in the error so a user knows which box to fix.
fn build_regex(
    pattern: &str,
    field: &str,
    ignore_case: bool,
    multiline: bool,
    dotall: bool,
) -> Result<regex::Regex, String> {
    RegexBuilder::new(pattern)
        .case_insensitive(ignore_case)
        .multi_line(multiline)
        .dot_matches_new_line(dotall)
        .build()
        .map_err(|e| format!("invalid {field} '{pattern}': {e}"))
}

/// Render the split table in the requested output format.
fn render(output: Output, rows: &[Vec<String>], separator: &str, has_fields: bool) -> String {
    match output {
        Output::Lines => rows
            .iter()
            .map(|r| r.join("\t"))
            .collect::<Vec<_>>()
            .join("\n"),
        Output::Tsv => rows
            .iter()
            .map(|r| r.join("\t"))
            .collect::<Vec<_>>()
            .join("\n"),
        Output::Csv => rows
            .iter()
            .map(|r| r.iter().map(|f| csv_field(f)).collect::<Vec<_>>().join(","))
            .collect::<Vec<_>>()
            .join("\n"),
        Output::Numbered => rows
            .iter()
            .enumerate()
            .map(|(i, r)| format!("{}. {}", i + 1, r.join("\t")))
            .collect::<Vec<_>>()
            .join("\n"),
        Output::Json => {
            // Rows only → a flat array of strings; fields split → an array of arrays.
            if has_fields {
                let body = rows
                    .iter()
                    .map(|r| {
                        let inner = r
                            .iter()
                            .map(|f| json_string(f))
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("  [{inner}]")
                    })
                    .collect::<Vec<_>>()
                    .join(",\n");
                format!("[\n{body}\n]")
            } else {
                let body = rows
                    .iter()
                    .map(|r| format!("  {}", json_string(&r.join("\t"))))
                    .collect::<Vec<_>>()
                    .join(",\n");
                format!("[\n{body}\n]")
            }
        }
        Output::Separator => {
            let sep = unescape(separator);
            rows.iter()
                .map(|r| r.join("\t"))
                .collect::<Vec<_>>()
                .join(&sep)
        }
    }
}

/// Quote a CSV field per RFC 4180: quote when it holds a comma, a quote, CR or
/// LF, and double any embedded quotes.
fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Serialise a string as a JSON string literal (escaping quotes, backslashes and
/// the control characters JSON requires).
fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
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
    out.push('"');
    out
}

/// Translate the backslash escapes a user is likely to type into a single-line
/// separator field. Anything else after a `\` is kept verbatim.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Convenience wrapper with the descriptor defaults.
    fn s(text: &str, pattern: &str) -> Result<String, String> {
        split(
            text, pattern, "", false, false, false, false, false, 0, "lines", ", ",
        )
    }

    #[test]
    fn splits_on_whitespace_runs() {
        assert_eq!(
            s("alpha   beta\t\tgamma", r"\s+").unwrap(),
            "alpha\nbeta\ngamma"
        );
    }

    #[test]
    fn splits_on_several_delimiters_at_once() {
        assert_eq!(s("a,b;c|d", r"[,;|]").unwrap(), "a\nb\nc\nd");
    }

    #[test]
    fn multi_character_separator() {
        assert_eq!(s("a -- b -- c", r" -- ").unwrap(), "a\nb\nc");
    }

    #[test]
    fn blank_line_paragraph_split() {
        assert_eq!(
            s("one\n\n\ntwo\n\nthree", r"\n{2,}").unwrap(),
            "one\ntwo\nthree"
        );
    }

    #[test]
    fn field_pattern_builds_a_table() {
        let out = split(
            "a:1\nb:2", r"\n", ":", false, false, false, false, false, 0, "tsv", ", ",
        )
        .unwrap();
        assert_eq!(out, "a\t1\nb\t2");
    }

    #[test]
    fn csv_output_quotes_per_rfc4180() {
        let out = split(
            "say \"hi\", now|next",
            r"\|",
            "",
            false,
            false,
            false,
            false,
            false,
            0,
            "csv",
            ", ",
        )
        .unwrap();
        assert_eq!(out, "\"say \"\"hi\"\", now\"\nnext");
    }

    #[test]
    fn json_output_is_a_flat_array_without_fields() {
        let out = split(
            "a,b", ",", "", false, false, false, false, false, 0, "json", ", ",
        )
        .unwrap();
        assert_eq!(out, "[\n  \"a\",\n  \"b\"\n]");
    }

    #[test]
    fn json_output_nests_when_fields_are_split() {
        let out = split(
            "a:1\nb:2", r"\n", ":", false, false, false, false, false, 0, "json", ", ",
        )
        .unwrap();
        assert_eq!(out, "[\n  [\"a\", \"1\"],\n  [\"b\", \"2\"]\n]");
    }

    #[test]
    fn numbered_output_counts_from_one() {
        let out = split(
            "a,b,c", ",", "", false, false, false, false, false, 0, "numbered", ", ",
        )
        .unwrap();
        assert_eq!(out, "1. a\n2. b\n3. c");
    }

    #[test]
    fn separator_output_unescapes_the_join_string() {
        let out = split(
            "a,b,c", ",", "", false, false, false, false, false, 0, "separator", " \\t ",
        )
        .unwrap();
        assert_eq!(out, "a \t b \t c");
    }

    #[test]
    fn trim_and_remove_empty_clean_up_parts() {
        let out = split(
            " a , b ,, c ",
            ",",
            "",
            false,
            false,
            false,
            true,
            true,
            0,
            "lines",
            ", ",
        )
        .unwrap();
        assert_eq!(out, "a\nb\nc");
    }

    #[test]
    fn max_splits_keeps_the_remainder_intact() {
        let out = split(
            "a,b,c,d", ",", "", false, false, false, false, false, 2, "lines", ", ",
        )
        .unwrap();
        assert_eq!(out, "a\nb\nc,d");
    }

    #[test]
    fn ignore_case_flag_applies_to_the_pattern() {
        let out = split(
            "1AND2and3",
            "and",
            "",
            true,
            false,
            false,
            false,
            false,
            0,
            "lines",
            ", ",
        )
        .unwrap();
        assert_eq!(out, "1\n2\n3");
    }

    #[test]
    fn multiline_flag_anchors_per_line() {
        // `^-+$` only matches the middle line when multiline is on.
        let out = split(
            "one\n---\ntwo",
            r"\n^-+$\n",
            "",
            false,
            true,
            false,
            false,
            false,
            0,
            "lines",
            ", ",
        )
        .unwrap();
        assert_eq!(out, "one\ntwo");
    }

    #[test]
    fn dotall_flag_lets_the_separator_span_lines() {
        let out = split(
            "a<START\nEND>b",
            "<.+>",
            "",
            false,
            false,
            true,
            false,
            false,
            0,
            "lines",
            ", ",
        )
        .unwrap();
        assert_eq!(out, "a\nb");
    }

    #[test]
    fn invalid_pattern_is_an_error() {
        let err = s("abc", "(unclosed").unwrap_err();
        assert!(err.contains("invalid pattern"), "unexpected error: {err}");
    }

    #[test]
    fn invalid_field_pattern_names_that_field() {
        let err = split(
            "a", r"\n", "[bad", false, false, false, false, false, 0, "lines", ", ",
        )
        .unwrap_err();
        assert!(
            err.contains("invalid field_pattern"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn empty_pattern_is_an_error() {
        let err = s("abc", "").unwrap_err();
        assert!(err.contains("pattern is empty"), "unexpected error: {err}");
    }

    #[test]
    fn empty_text_is_an_error() {
        let err = s("", ",").unwrap_err();
        assert!(err.contains("text is empty"), "unexpected error: {err}");
    }

    #[test]
    fn unknown_output_format_is_an_error() {
        let err = split(
            "a,b", ",", "", false, false, false, false, false, 0, "xml", ", ",
        )
        .unwrap_err();
        assert!(
            err.contains("unknown output format"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn text_over_the_character_cap_is_rejected() {
        let big = "x".repeat(MAX_TEXT_CHARS + 1);
        let err = s(&big, ",").unwrap_err();
        assert!(err.contains("character limit"), "unexpected error: {err}");
    }

    #[test]
    fn text_at_the_character_cap_is_accepted() {
        let at_cap = "x".repeat(MAX_TEXT_CHARS);
        assert!(s(&at_cap, ",").is_ok());
    }

    #[test]
    fn too_many_parts_is_an_error() {
        // An empty-width match splits at every position.
        let err = s(&"a".repeat(MAX_PARTS), "").unwrap_err();
        // An empty pattern is caught earlier; use a zero-width assertion instead.
        assert!(err.contains("pattern is empty"), "unexpected error: {err}");
        let err = s(&"a".repeat(MAX_PARTS + 1), "b*").unwrap_err();
        assert!(err.contains("more than"), "unexpected error: {err}");
    }

    #[test]
    fn everything_removed_is_an_error() {
        let err = split(
            "  ,  ", ",", "", false, false, false, true, true, 0, "lines", ", ",
        )
        .unwrap_err();
        assert!(err.contains("no parts left"), "unexpected error: {err}");
    }

    #[test]
    fn unicode_characters_count_not_bytes() {
        // 3-byte characters: the cap is measured in chars, so this passes.
        let text = "あ".repeat(MAX_TEXT_CHARS);
        assert!(s(&text, ",").is_ok());
    }
}
