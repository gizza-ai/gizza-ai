//! gizza-ai/csv-regex-replace core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! Applies ONE find-and-replace rule to the cells of selected columns in a parsed
//! CSV/delimited table, with capture-group substitution in the replacement.
//!
//! The table is parsed first, so a pattern only ever sees a decoded cell value —
//! it can never match across a delimiter, eat a quote character, or split a
//! quoted field that contains an embedded newline. Output quoting is re-derived
//! from the resulting values.
//!
//! Distinct from `find-replace` (same rule over free text, no CSV/column
//! awareness), `regex-column-validate` (reports matches, never rewrites) and
//! `csv-null-standardizer` (fixed literal token list, no captures).

use csv::{QuoteStyle, ReaderBuilder, StringRecord, WriterBuilder};
use regex::{NoExpand, RegexBuilder};

/// Hard cap on the input table, mirroring the sibling CSV blocks.
pub const MAX_INPUT_BYTES: usize = 5_000_000;

/// How the pattern is interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `pattern` is a Rust regular expression; `replacement` expands `$1`/`${name}`.
    Regex,
    /// `pattern` is matched literally; `replacement` is inserted verbatim.
    Literal,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "regex" => Ok(Mode::Regex),
            "literal" => Ok(Mode::Literal),
            other => Err(format!(
                "unknown mode '{other}' (use 'regex' or 'literal')"
            )),
        }
    }
}

/// Whether the pattern may match part of a cell or must match all of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchScope {
    /// Match anywhere inside the cell value.
    Substring,
    /// The pattern must match the entire cell value.
    WholeCell,
}

impl MatchScope {
    pub fn parse(s: &str) -> Result<MatchScope, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "substring" => Ok(MatchScope::Substring),
            "whole_cell" | "whole-cell" | "cell" => Ok(MatchScope::WholeCell),
            other => Err(format!(
                "unknown match_scope '{other}' (use 'substring' or 'whole_cell')"
            )),
        }
    }
}

/// What the tool returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// The whole table with the replacements applied.
    Csv,
    /// Only the rows that changed (plus the header, when there is one).
    Changed,
    /// A per-column `column,cells_changed,replacements` audit table.
    Report,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "csv" => Ok(Output::Csv),
            "changed" => Ok(Output::Changed),
            "report" => Ok(Output::Report),
            other => Err(format!(
                "unknown output '{other}' (use 'csv', 'changed', or 'report')"
            )),
        }
    }
}

fn quote_style(s: &str) -> Result<QuoteStyle, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "minimal" => Ok(QuoteStyle::Necessary),
        "always" => Ok(QuoteStyle::Always),
        "non_numeric" | "non-numeric" => Ok(QuoteStyle::NonNumeric),
        other => Err(format!(
            "unknown quote_style '{other}' (use 'minimal', 'always', or 'non_numeric')"
        )),
    }
}

/// Resolve a delimiter spec (`auto` is handled by the caller) to a single byte.
fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d.trim() {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 {
                b[0]
            } else {
                return Err(format!(
                    "delimiter must be 'auto', a single character, or comma/tab/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// Count a candidate separator on the first line, ignoring anything inside quotes.
fn count_outside_quotes(line: &str, sep: char) -> usize {
    let mut in_quotes = false;
    let mut n = 0;
    for c in line.chars() {
        if c == '"' {
            in_quotes = !in_quotes;
        } else if c == sep && !in_quotes {
            n += 1;
        }
    }
    n
}

/// Sniff the separator from the first line: most frequent candidate outside
/// quotes, comma winning any tie.
fn sniff_delimiter(data: &str) -> u8 {
    let first = data.lines().next().unwrap_or("");
    let mut best = b',';
    let mut best_n = 0;
    for (c, b) in [(',', b','), ('\t', b'\t'), (';', b';'), ('|', b'|')] {
        let n = count_outside_quotes(first, c);
        if n > best_n {
            best_n = n;
            best = b;
        }
    }
    best
}

/// Resolve the `columns` spec into a per-column selected flag.
///
/// A blank spec selects EVERY column. Each comma-separated entry is matched
/// against a header name first (exact, then case-insensitive), then read as a
/// 1-based index or an inclusive `2-4` index range.
fn resolve_columns(
    spec: &str,
    header: Option<&StringRecord>,
    width: usize,
) -> Result<Vec<bool>, String> {
    let trimmed = spec.trim();
    if trimmed.is_empty() || trimmed == "*" {
        return Ok(vec![true; width]);
    }
    let mut selected = vec![false; width];
    for raw in trimmed.split(',') {
        let key = raw.trim();
        if key.is_empty() {
            continue;
        }
        if let Some(idx) = match_header(key, header) {
            selected[idx] = true;
            continue;
        }
        // A `2-4` range, but only when both sides are plain numbers — a header
        // named "first-name" already matched above.
        if let Some((lo, hi)) = key.split_once('-') {
            let (lo, hi) = (lo.trim(), hi.trim());
            if !lo.is_empty()
                && !hi.is_empty()
                && lo.chars().all(|c| c.is_ascii_digit())
                && hi.chars().all(|c| c.is_ascii_digit())
            {
                let lo = parse_index(lo, width)?;
                let hi = parse_index(hi, width)?;
                if lo > hi {
                    return Err(format!(
                        "column range '{key}' runs backwards (write it low-to-high)"
                    ));
                }
                for item in selected.iter_mut().take(hi + 1).skip(lo) {
                    *item = true;
                }
                continue;
            }
        }
        let idx = match parse_index(key, width) {
            Ok(i) => i,
            // A numeric key that failed is out of range — keep that precise message.
            Err(e) if key.chars().all(|c| c.is_ascii_digit()) => return Err(e),
            Err(e) => {
                return Err(match header {
                    Some(h) => {
                        let names: Vec<&str> = h.iter().collect();
                        format!(
                            "no column named '{key}' and it is not a valid index — available: {}",
                            names.join(", ")
                        )
                    }
                    None => format!(
                        "{e} (there is no header row, so columns must be 1-based indices or ranges)"
                    ),
                })
            }
        };
        selected[idx] = true;
    }
    if !selected.iter().any(|s| *s) {
        return Err("no columns selected — leave 'columns' blank to replace in every column".into());
    }
    Ok(selected)
}

fn match_header(key: &str, header: Option<&StringRecord>) -> Option<usize> {
    let hdr = header?;
    for (i, name) in hdr.iter().enumerate() {
        if name.trim() == key {
            return Some(i);
        }
    }
    for (i, name) in hdr.iter().enumerate() {
        if name.trim().eq_ignore_ascii_case(key) {
            return Some(i);
        }
    }
    None
}

/// Parse a 1-based column index into a 0-based offset, bounds-checked.
fn parse_index(key: &str, width: usize) -> Result<usize, String> {
    let n: usize = key
        .parse()
        .map_err(|_| format!("column must be a name, a 1-based index, or a range like 2-4, got '{key}'"))?;
    if n == 0 || n > width {
        return Err(format!(
            "column index {n} out of range (the table has {width} column(s))"
        ));
    }
    Ok(n - 1)
}

/// Apply a regex find-and-replace across the selected columns of a CSV table.
///
/// * `data` — the CSV/delimited table text (max [`MAX_INPUT_BYTES`]).
/// * `pattern` — a Rust regular expression, or a literal string under `mode = "literal"`.
/// * `replacement` — the replacement text; in regex mode `$1`/`${name}`/`$0` expand
///   capture groups and `$$` is a literal `$`. Blank deletes every match.
/// * `columns` — blank (or `*`) for every column, else names / 1-based indices / `2-4` ranges.
/// * `mode` — `regex` (default) or `literal`.
/// * `match_scope` — `substring` (default) or `whole_cell`.
/// * `ignore_case`, `multiline`, `dotall` — the `i`, `m` and `s` regex flags.
/// * `replace_all` — replace every match in a cell (default) or only the first.
/// * `has_header` — treat row 1 as a header (default true).
/// * `include_header` — also apply the rule to the header row (default false).
/// * `delimiter` — `auto`, a single character, or comma/tab/semicolon/pipe.
/// * `quote_style` — `minimal` (default), `always`, or `non_numeric`.
/// * `output` — `csv` (default), `changed`, or `report`.
#[allow(clippy::too_many_arguments)]
pub fn replace(
    data: &str,
    pattern: &str,
    replacement: &str,
    columns: &str,
    mode: &str,
    match_scope: &str,
    ignore_case: bool,
    multiline: bool,
    dotall: bool,
    replace_all: bool,
    has_header: bool,
    include_header: bool,
    delimiter: &str,
    quote_style_spec: &str,
    output: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("no CSV data provided".into());
    }
    if data.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes, over the {MAX_INPUT_BYTES}-byte limit",
            data.len()
        ));
    }
    if pattern.is_empty() {
        return Err("no pattern provided — enter the text or regular expression to find".into());
    }

    let mode = Mode::parse(mode)?;
    let scope = MatchScope::parse(match_scope)?;
    let output = Output::parse(output)?;
    let qstyle = quote_style(quote_style_spec)?;
    let delim = if delimiter.trim().eq_ignore_ascii_case("auto") {
        sniff_delimiter(data)
    } else {
        delim_byte(delimiter)?
    };

    let re = build_regex(pattern, mode, scope, ignore_case, multiline, dotall)?;

    let mut rdr = ReaderBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .has_headers(false)
        .from_reader(data.as_bytes());

    let mut records: Vec<StringRecord> = Vec::new();
    for rec in rdr.records() {
        records.push(rec.map_err(|e| format!("could not parse the CSV: {e}"))?);
    }
    if records.is_empty() {
        return Err("no CSV rows found".into());
    }

    let width = records.iter().map(|r| r.len()).max().unwrap_or(0);
    let header = if has_header { Some(&records[0]) } else { None };
    let selected = resolve_columns(columns, header, width)?;

    let first_data_row = usize::from(has_header && !include_header);
    let mut cells_changed = vec![0usize; width];
    let mut replacements = vec![0usize; width];
    let mut changed_rows: Vec<bool> = vec![false; records.len()];

    let mut out_records: Vec<StringRecord> = Vec::with_capacity(records.len());
    for (row, rec) in records.iter().enumerate() {
        if row < first_data_row {
            out_records.push(rec.clone());
            continue;
        }
        let mut fields: Vec<String> = Vec::with_capacity(rec.len());
        for (col, value) in rec.iter().enumerate() {
            if col >= selected.len() || !selected[col] {
                fields.push(value.to_string());
                continue;
            }
            let (new_value, hits) = apply(&re, value, replacement, mode, replace_all);
            if hits > 0 && new_value != value {
                cells_changed[col] += 1;
                replacements[col] += hits;
                changed_rows[row] = true;
            } else if hits > 0 {
                // A match that produced identical text still counts as a replacement.
                replacements[col] += hits;
            }
            fields.push(new_value);
        }
        out_records.push(StringRecord::from(fields));
    }

    match output {
        Output::Csv => write_csv(delim, qstyle, out_records.iter()),
        Output::Changed => {
            let mut keep: Vec<&StringRecord> = Vec::new();
            if has_header {
                keep.push(&out_records[0]);
            }
            for (row, rec) in out_records.iter().enumerate() {
                if row == 0 && has_header {
                    continue;
                }
                if changed_rows[row] {
                    keep.push(rec);
                }
            }
            write_csv(delim, qstyle, keep.into_iter())
        }
        Output::Report => {
            let mut rows: Vec<StringRecord> = vec![StringRecord::from(vec![
                "column",
                "cells_changed",
                "replacements",
            ])];
            for (col, sel) in selected.iter().enumerate() {
                if !sel {
                    continue;
                }
                let name = match header {
                    Some(h) if col < h.len() && !h[col].trim().is_empty() => h[col].to_string(),
                    _ => (col + 1).to_string(),
                };
                rows.push(StringRecord::from(vec![
                    name,
                    cells_changed[col].to_string(),
                    replacements[col].to_string(),
                ]));
            }
            rows.push(StringRecord::from(vec![
                "TOTAL".to_string(),
                cells_changed.iter().sum::<usize>().to_string(),
                replacements.iter().sum::<usize>().to_string(),
            ]));
            write_csv(delim, qstyle, rows.iter())
        }
    }
}

/// Compile the pattern, applying the mode, scope and flag options.
fn build_regex(
    pattern: &str,
    mode: Mode,
    scope: MatchScope,
    ignore_case: bool,
    multiline: bool,
    dotall: bool,
) -> Result<regex::Regex, String> {
    let base = match mode {
        Mode::Regex => pattern.to_string(),
        Mode::Literal => regex::escape(pattern),
    };
    // Whole-cell anchors the pattern against the entire value; `\A`/`\z` are used
    // rather than `^`/`$` so the multiline flag cannot loosen the anchoring.
    let source = match scope {
        MatchScope::Substring => base,
        MatchScope::WholeCell => format!(r"\A(?:{base})\z"),
    };
    RegexBuilder::new(&source)
        .case_insensitive(ignore_case)
        .multi_line(multiline)
        .dot_matches_new_line(dotall)
        .build()
        .map_err(|e| format!("invalid pattern: {e}"))
}

/// Replace within one cell, returning the new value and the number of matches
/// that were substituted.
fn apply(
    re: &regex::Regex,
    value: &str,
    replacement: &str,
    mode: Mode,
    replace_all: bool,
) -> (String, usize) {
    let limit = if replace_all { 0 } else { 1 };
    let hits = if replace_all {
        re.find_iter(value).count()
    } else {
        usize::from(re.is_match(value))
    };
    if hits == 0 {
        return (value.to_string(), 0);
    }
    let out = match mode {
        Mode::Regex => re.replacen(value, limit, replacement).into_owned(),
        Mode::Literal => re
            .replacen(value, limit, NoExpand(replacement))
            .into_owned(),
    };
    (out, hits)
}

fn write_csv<'a, I: Iterator<Item = &'a StringRecord>>(
    delim: u8,
    qstyle: QuoteStyle,
    records: I,
) -> Result<String, String> {
    let mut wtr = WriterBuilder::new()
        .delimiter(delim)
        .quote_style(qstyle)
        .flexible(true)
        .from_writer(vec![]);
    for rec in records {
        wtr.write_record(rec)
            .map_err(|e| format!("could not write the CSV: {e}"))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("could not write the CSV: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("output was not valid UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Defaults: regex mode, every column, substring, replace-all, header kept.
    #[allow(clippy::too_many_arguments)]
    fn run(data: &str, pattern: &str, replacement: &str, columns: &str) -> Result<String, String> {
        replace(
            data,
            pattern,
            replacement,
            columns,
            "regex",
            "substring",
            false,
            false,
            false,
            true,
            true,
            false,
            ",",
            "minimal",
            "csv",
        )
    }

    #[test]
    fn happy_path_capture_group_swap_in_one_column() {
        let out = run(
            "name,city\n\"Lovelace, Ada\",Paris\n\"Hopper, Grace\",Boston\n",
            r"(\w+), (\w+)",
            "$2 $1",
            "name",
        )
        .unwrap();
        assert_eq!(out, "name,city\nAda Lovelace,Paris\nGrace Hopper,Boston\n");
    }

    #[test]
    fn other_columns_are_untouched() {
        let out = run("a,b\nxx,xx\n", "x", "y", "b").unwrap();
        assert_eq!(out, "a,b\nxx,yy\n");
    }

    #[test]
    fn header_row_is_not_searched_by_default() {
        let out = run("code,code2\ncode,x\n", "code", "ID", "").unwrap();
        assert_eq!(out, "code,code2\nID,x\n");
    }

    #[test]
    fn include_header_rewrites_the_header_too() {
        let out = replace(
            "code,other\ncode,x\n",
            "code",
            "ID",
            "",
            "regex",
            "substring",
            false,
            false,
            false,
            true,
            true,
            true,
            ",",
            "minimal",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "ID,other\nID,x\n");
    }

    #[test]
    fn blank_columns_selects_every_column() {
        let out = run("a,b\n1-2,3-4\n", "-", "/", "").unwrap();
        assert_eq!(out, "a,b\n1/2,3/4\n");
    }

    #[test]
    fn index_and_range_column_specs() {
        let out = run("a,b,c,d\nx,x,x,x\n", "x", "y", "2-3").unwrap();
        assert_eq!(out, "a,b,c,d\nx,y,y,x\n");
        let out = run("a,b,c,d\nx,x,x,x\n", "x", "y", "1,4").unwrap();
        assert_eq!(out, "a,b,c,d\ny,x,x,y\n");
    }

    #[test]
    fn column_names_match_case_insensitively_as_a_fallback() {
        let out = run("Name,City\nada,paris\n", "a", "A", "name").unwrap();
        assert_eq!(out, "Name,City\nAdA,paris\n");
    }

    #[test]
    fn literal_mode_escapes_the_pattern_and_takes_the_replacement_verbatim() {
        let out = replace(
            "a\n1.5\n",
            ".",
            "$1,",
            "",
            "literal",
            "substring",
            false,
            false,
            false,
            true,
            true,
            false,
            ",",
            "minimal",
            "csv",
        )
        .unwrap();
        // `.` matched literally (not "any char"), and `$1` is inserted as text.
        assert_eq!(out, "a\n\"1$1,5\"\n");
    }

    #[test]
    fn whole_cell_scope_only_matches_a_full_value() {
        let out = replace(
            "a\nNA\nNAME\n",
            "NA",
            "",
            "",
            "regex",
            "whole_cell",
            false,
            false,
            false,
            true,
            true,
            false,
            ",",
            "minimal",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "a\n\"\"\nNAME\n");
    }

    #[test]
    fn replace_first_only_touches_the_first_match_per_cell() {
        let out = replace(
            "a\nx-x-x\n",
            "x",
            "y",
            "",
            "regex",
            "substring",
            false,
            false,
            false,
            false,
            true,
            false,
            ",",
            "minimal",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "a\ny-x-x\n");
    }

    #[test]
    fn ignore_case_flag() {
        let out = replace(
            "a\nHello\n",
            "hello",
            "hi",
            "",
            "regex",
            "substring",
            true,
            false,
            false,
            true,
            true,
            false,
            ",",
            "minimal",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "a\nhi\n");
    }

    #[test]
    fn dotall_lets_a_pattern_span_an_embedded_newline() {
        let out = replace(
            "a\n\"one\ntwo\"\n",
            "one.two",
            "merged",
            "",
            "regex",
            "substring",
            false,
            false,
            true,
            true,
            true,
            false,
            ",",
            "minimal",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "a\nmerged\n");
    }

    #[test]
    fn multiline_anchors_inside_an_embedded_newline_cell() {
        let out = replace(
            "a\n\"one\ntwo\"\n",
            "^t",
            "T",
            "",
            "regex",
            "substring",
            false,
            true,
            false,
            true,
            true,
            false,
            ",",
            "minimal",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "a\n\"one\nTwo\"\n");
    }

    #[test]
    fn a_delimiter_inside_a_replacement_gets_requoted() {
        let out = run("a,b\nx,y\n", "x", "1,2", "a").unwrap();
        assert_eq!(out, "a,b\n\"1,2\",y\n");
    }

    #[test]
    fn tab_delimited_input_round_trips() {
        let out = run("a\tb\nxx\txx\n", "x", "y", "auto").unwrap_err();
        // 'auto' is not a column name here — the delimiter option is separate.
        assert!(out.contains("no column named 'auto'"), "{out}");
        let out = replace(
            "a\tb\nxx\txx\n",
            "x",
            "y",
            "b",
            "regex",
            "substring",
            false,
            false,
            false,
            true,
            true,
            false,
            "auto",
            "minimal",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "a\tb\nxx\tyy\n");
    }

    #[test]
    fn changed_output_keeps_only_affected_rows() {
        let out = replace(
            "id,note\n1,keep\n2,fix me\n3,keep\n",
            "fix",
            "fixed",
            "note",
            "regex",
            "substring",
            false,
            false,
            false,
            true,
            true,
            false,
            ",",
            "minimal",
            "changed",
        )
        .unwrap();
        assert_eq!(out, "id,note\n2,fixed me\n");
    }

    #[test]
    fn report_output_counts_per_column_and_total() {
        let out = replace(
            "a,b\nxx,x\nx,\n",
            "x",
            "y",
            "",
            "regex",
            "substring",
            false,
            false,
            false,
            true,
            true,
            false,
            ",",
            "minimal",
            "report",
        )
        .unwrap();
        assert_eq!(
            out,
            "column,cells_changed,replacements\na,2,3\nb,1,1\nTOTAL,3,4\n"
        );
    }

    #[test]
    fn quote_style_always_quotes_every_field() {
        let out = replace(
            "a,b\nx,y\n",
            "x",
            "z",
            "",
            "regex",
            "substring",
            false,
            false,
            false,
            true,
            true,
            false,
            ",",
            "always",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "\"a\",\"b\"\n\"z\",\"y\"\n");
    }

    #[test]
    fn no_header_mode_treats_row_one_as_data() {
        let out = replace(
            "x,x\nx,x\n",
            "x",
            "y",
            "1",
            "regex",
            "substring",
            false,
            false,
            false,
            true,
            false,
            false,
            ",",
            "minimal",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "y,x\ny,x\n");
    }

    #[test]
    fn ragged_rows_are_preserved() {
        let out = run("a,b,c\n1,2\n3,4,5,6\n", "\\d", "n", "").unwrap();
        assert_eq!(out, "a,b,c\nn,n\nn,n,n,n\n");
    }

    #[test]
    fn err_on_invalid_regex() {
        let e = run("a\n1\n", "(", "x", "").unwrap_err();
        assert!(e.starts_with("invalid pattern:"), "{e}");
    }

    #[test]
    fn err_on_empty_input() {
        assert_eq!(run("   ", "x", "y", "").unwrap_err(), "no CSV data provided");
    }

    #[test]
    fn err_on_empty_pattern() {
        let e = run("a\n1\n", "", "y", "").unwrap_err();
        assert!(e.starts_with("no pattern provided"), "{e}");
    }

    #[test]
    fn err_on_unknown_column_lists_the_available_names() {
        let e = run("name,city\nada,paris\n", "a", "b", "nope").unwrap_err();
        assert!(e.contains("no column named 'nope'"), "{e}");
        assert!(e.contains("available: name, city"), "{e}");
    }

    #[test]
    fn err_on_out_of_range_index() {
        let e = run("a,b\n1,2\n", "1", "x", "5").unwrap_err();
        assert!(e.contains("out of range"), "{e}");
    }

    #[test]
    fn err_on_backwards_range() {
        let e = run("a,b,c\n1,2,3\n", "1", "x", "3-1").unwrap_err();
        assert!(e.contains("runs backwards"), "{e}");
    }

    #[test]
    fn err_on_unknown_mode_and_scope_and_output() {
        assert!(replace(
            "a\n1\n", "1", "x", "", "nope", "substring", false, false, false, true, true, false,
            ",", "minimal", "csv"
        )
        .unwrap_err()
        .contains("unknown mode"));
        assert!(replace(
            "a\n1\n", "1", "x", "", "regex", "nope", false, false, false, true, true, false, ",",
            "minimal", "csv"
        )
        .unwrap_err()
        .contains("unknown match_scope"));
        assert!(replace(
            "a\n1\n", "1", "x", "", "regex", "substring", false, false, false, true, true, false,
            ",", "minimal", "nope"
        )
        .unwrap_err()
        .contains("unknown output"));
    }

    #[test]
    fn err_on_multi_char_delimiter() {
        let e = replace(
            "a\n1\n", "1", "x", "", "regex", "substring", false, false, false, true, true, false,
            "::", "minimal", "csv",
        )
        .unwrap_err();
        assert!(e.contains("delimiter must be"), "{e}");
    }

    #[test]
    fn err_over_the_size_cap() {
        let big = format!("a\n{}\n", "x".repeat(MAX_INPUT_BYTES));
        let e = run(&big, "x", "y", "").unwrap_err();
        assert!(e.contains("over the 5000000-byte limit"), "{e}");
    }

    #[test]
    fn blank_replacement_deletes_matches() {
        // The value is a quoted field, so the comma inside it is data, not a
        // separator — the pattern only ever sees the decoded `$1,200`.
        let out = run("price\n\"$1,200\"\n", r"[$,]", "", "").unwrap();
        assert_eq!(out, "price\n1200\n");
    }
}
