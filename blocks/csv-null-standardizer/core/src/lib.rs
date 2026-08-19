//! csv-null-standardizer core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps. Rewrites every cell that represents a
//! missing value — a blank cell, or one of the assorted "no data" tokens people
//! actually ship in CSVs (`NA`, `N/A`, `#N/A`, `NULL`, `NaN`, `None`, `<NA>`, `-`,
//! `?`, …) — into ONE chosen representation, so a downstream loader has a single
//! rule to configure instead of a dozen.
//!
//! Everything else is preserved: non-missing cells are copied verbatim (no
//! trimming, no re-typing), the header row is never touched, and the field
//! separator round-trips unchanged. This normalizes the *token*; it never invents
//! a value (that is imputation, a different job).

use std::collections::HashSet;

/// Hard cap on the pasted table, so a runaway paste can't wedge the tab.
pub const MAX_INPUT_BYTES: usize = 5_000_000;

/// The built-in missing-token vocabulary. Modeled on the conventions that show up
/// across pandas/R/Excel exports, plus the bare dashes and question marks that
/// hand-maintained sheets use. Matched case-insensitively by default, so one
/// entry covers `NULL`/`null`/`Null`.
pub const DEFAULT_NA_TOKENS: &str = "NA,N/A,N.A.,#N/A,#N/A N/A,#NA,NULL,NIL,NaN,None,<NA>,-,--,?";

/// Resolve a delimiter spec to its byte. `auto` sniffs the first line.
fn delim_byte(spec: &str, data: &str) -> Result<u8, String> {
    let s = spec.trim();
    Ok(match s {
        "auto" => sniff_delimiter(data),
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

/// Pick the delimiter that occurs most often outside quotes on the first
/// non-empty line. Ties (and a line with none of them) fall back to a comma.
fn sniff_delimiter(data: &str) -> u8 {
    let line = data.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut in_quote = false;
    let (mut comma, mut tab, mut semi, mut pipe) = (0usize, 0usize, 0usize, 0usize);
    for ch in line.chars() {
        match ch {
            '"' => in_quote = !in_quote,
            _ if in_quote => {}
            ',' => comma += 1,
            '\t' => tab += 1,
            ';' => semi += 1,
            '|' => pipe += 1,
            _ => {}
        }
    }
    // Comma first so it wins every tie.
    [(b',', comma), (b'\t', tab), (b';', semi), (b'|', pipe)]
        .into_iter()
        .filter(|(_, n)| *n > 0)
        .max_by_key(|(_, n)| *n)
        .map(|(b, _)| b)
        .unwrap_or(b',')
}

/// Output quoting policy.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Quoting {
    /// Quote only where the CSV grammar requires it (default).
    Minimal,
    /// Quote every field, the replacement token included — for readers that
    /// expect uniformly quoted CSV.
    Always,
    /// Never quote. Fast and ugly; can emit ambiguous CSV, see the docs.
    Never,
}

fn parse_quoting(s: &str) -> Result<Quoting, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "minimal" => Ok(Quoting::Minimal),
        "always" => Ok(Quoting::Always),
        "never" => Ok(Quoting::Never),
        other => Err(format!(
            "quote_style must be 'minimal', 'always', or 'never', got '{other}'"
        )),
    }
}

/// Split the token list on commas, trimming each entry and dropping empties.
fn parse_tokens(spec: &str, case_sensitive: bool) -> HashSet<String> {
    spec.split(',')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(|t| {
            if case_sensitive {
                t.to_string()
            } else {
                t.to_lowercase()
            }
        })
        .collect()
}

/// Resolve the `columns` selector into the set of 0-based column indices to
/// rewrite. An empty selector means "every column".
fn parse_columns(
    spec: &str,
    header_row: Option<&[String]>,
) -> Result<Option<HashSet<usize>>, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Ok(None);
    }
    let mut out = HashSet::new();
    for raw in spec.split(',') {
        let name = raw.trim();
        if name.is_empty() {
            continue;
        }
        // A bare positive integer is always a 1-based column position.
        if let Ok(idx) = name.parse::<usize>() {
            if idx == 0 {
                return Err("column positions are 1-based, got '0'".to_string());
            }
            if let Some(h) = header_row {
                if idx > h.len() {
                    return Err(format!(
                        "column position {idx} is out of range (the table has {} columns)",
                        h.len()
                    ));
                }
            }
            out.insert(idx - 1);
            continue;
        }
        let Some(h) = header_row else {
            return Err(format!(
                "column '{name}' is a name, but the header option is off — use 1-based column positions instead"
            ));
        };
        let found = h
            .iter()
            .position(|c| c.trim() == name)
            .or_else(|| h.iter().position(|c| c.trim().eq_ignore_ascii_case(name)));
        match found {
            Some(i) => {
                out.insert(i);
            }
            None => {
                return Err(format!(
                    "column '{name}' is not in the header (available: {})",
                    h.iter()
                        .map(|c| c.trim())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            }
        }
    }
    if out.is_empty() {
        return Err("columns selector resolved to no columns".to_string());
    }
    Ok(Some(out))
}

/// True when this cell represents a missing value under the current options.
fn is_missing(cell: &str, tokens: &HashSet<String>, blank_is_missing: bool, case_sensitive: bool, trim: bool) -> bool {
    if blank_is_missing && cell.trim().is_empty() {
        return true;
    }
    if tokens.is_empty() {
        return false;
    }
    let probe = if trim { cell.trim() } else { cell };
    if case_sensitive {
        tokens.contains(probe)
    } else {
        tokens.contains(&probe.to_lowercase())
    }
}

/// Standardize every missing-value token in a CSV/delimited table.
///
/// * `data` — the table text.
/// * `delimiter` — `auto`, a single character, or `comma`/`tab`/`semicolon`/`pipe`.
/// * `na_tokens` — comma-separated tokens that count as missing (see `DEFAULT_NA_TOKENS`).
/// * `replace_with` — the single representation written for every missing cell
///   (empty string = leave the cell blank).
/// * `blank_is_missing` — also normalize blank / whitespace-only cells.
/// * `case_sensitive` — match tokens exactly rather than case-insensitively.
/// * `trim` — ignore whitespace around a cell when matching (non-missing cells are
///   still copied verbatim).
/// * `header` — treat the first row as a header and never rewrite it.
/// * `columns` — comma-separated column names (needs a header) or 1-based
///   positions to rewrite; empty = every column.
/// * `quote_style` — `minimal` (default), `always`, or `never`.
#[allow(clippy::too_many_arguments)]
pub fn standardize(
    data: &str,
    delimiter: &str,
    na_tokens: &str,
    replace_with: &str,
    blank_is_missing: bool,
    case_sensitive: bool,
    trim: bool,
    header: bool,
    columns: &str,
    quote_style: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty — paste a CSV table with at least one row".to_string());
    }
    if data.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes, which exceeds the {MAX_INPUT_BYTES}-byte limit",
            data.len()
        ));
    }

    let delim = delim_byte(delimiter, data)?;
    let quoting = parse_quoting(quote_style)?;
    let tokens = parse_tokens(na_tokens, case_sensitive);

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .has_headers(false)
        .from_reader(data.as_bytes());

    let mut rows: Vec<Vec<String>> = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("CSV parse error: {e}"))?;
        rows.push(rec.iter().map(|f| f.to_string()).collect());
    }
    if rows.is_empty() {
        return Err("input is empty — paste a CSV table with at least one row".to_string());
    }

    let header_row = if header { Some(rows[0].clone()) } else { None };
    let selected = parse_columns(columns, header_row.as_deref())?;

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .quote_style(match quoting {
            Quoting::Minimal => csv::QuoteStyle::Necessary,
            Quoting::Always => csv::QuoteStyle::Always,
            Quoting::Never => csv::QuoteStyle::Never,
        })
        .flexible(true)
        .terminator(csv::Terminator::Any(b'\n'))
        .from_writer(Vec::new());

    for (r, row) in rows.iter().enumerate() {
        if header && r == 0 {
            wtr.write_record(row)
                .map_err(|e| format!("CSV write error: {e}"))?;
            continue;
        }
        let out: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(c, cell)| {
                let in_scope = selected.as_ref().is_none_or(|s| s.contains(&c));
                if in_scope && is_missing(cell, &tokens, blank_is_missing, case_sensitive, trim) {
                    replace_with.to_string()
                } else {
                    cell.clone()
                }
            })
            .collect();
        wtr.write_record(&out)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }

    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("output is not valid UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(data: &str, replace_with: &str) -> Result<String, String> {
        standardize(
            data,
            ",",
            DEFAULT_NA_TOKENS,
            replace_with,
            true,
            false,
            true,
            true,
            "",
            "minimal",
        )
    }

    #[test]
    fn standardizes_assorted_tokens_to_one_representation() {
        let got = run("id,score,notes\n1,42,ok\n2,NA,\n3,null,-\n4,n/a,NaN\n", "NULL").unwrap();
        assert_eq!(
            got,
            "id,score,notes\n1,42,ok\n2,NULL,NULL\n3,NULL,NULL\n4,NULL,NULL\n"
        );
    }

    #[test]
    fn blank_target_leaves_empty_cells() {
        let got = run("a,b\nNA,1\n-,2\n", "").unwrap();
        assert_eq!(got, "a,b\n,1\n,2\n");
    }

    #[test]
    fn header_row_is_never_rewritten() {
        // A column literally named "NA" survives; the data cell below it does not.
        let got = run("NA,b\nNA,1\n", "NULL").unwrap();
        assert_eq!(got, "NA,b\nNULL,1\n");
    }

    #[test]
    fn header_off_rewrites_the_first_row_too() {
        let got = standardize(
            "NA,1\nNA,2\n",
            ",",
            DEFAULT_NA_TOKENS,
            "NULL",
            true,
            false,
            true,
            false,
            "",
            "minimal",
        )
        .unwrap();
        assert_eq!(got, "NULL,1\nNULL,2\n");
    }

    #[test]
    fn non_missing_cells_are_copied_verbatim() {
        // trim only affects MATCHING — a padded real value keeps its padding.
        let got = run("a,b\n  hello  ,  NA  \n", "NULL").unwrap();
        assert_eq!(got, "a,b\n  hello  ,NULL\n");
    }

    #[test]
    fn trim_off_requires_an_exact_token() {
        let got = standardize(
            "a,b\n NA ,NA\n",
            ",",
            DEFAULT_NA_TOKENS,
            "NULL",
            true,
            false,
            false,
            true,
            "",
            "minimal",
        )
        .unwrap();
        assert_eq!(got, "a,b\n NA ,NULL\n");
    }

    #[test]
    fn case_sensitive_matching_keeps_other_casings() {
        let got = standardize(
            "a,b\nNA,na\n",
            ",",
            "NA",
            "NULL",
            true,
            true,
            true,
            true,
            "",
            "minimal",
        )
        .unwrap();
        assert_eq!(got, "a,b\nNULL,na\n");
    }

    #[test]
    fn blank_is_missing_off_keeps_empty_cells_empty() {
        let got = standardize(
            "a,b\nNA,\n",
            ",",
            DEFAULT_NA_TOKENS,
            "NULL",
            false,
            false,
            true,
            true,
            "",
            "minimal",
        )
        .unwrap();
        assert_eq!(got, "a,b\nNULL,\n");
    }

    #[test]
    fn empty_token_list_only_normalizes_blanks() {
        let got = standardize(
            "a,b\nNA,\n",
            ",",
            "",
            "NULL",
            true,
            false,
            true,
            true,
            "",
            "minimal",
        )
        .unwrap();
        assert_eq!(got, "a,b\nNA,NULL\n");
    }

    #[test]
    fn columns_selector_by_name_scopes_the_rewrite() {
        let got = standardize(
            "a,b\nNA,NA\n",
            ",",
            DEFAULT_NA_TOKENS,
            "NULL",
            true,
            false,
            true,
            true,
            "b",
            "minimal",
        )
        .unwrap();
        assert_eq!(got, "a,b\nNA,NULL\n");
    }

    #[test]
    fn columns_selector_accepts_one_based_positions() {
        let got = standardize(
            "a,b,c\nNA,NA,NA\n",
            ",",
            DEFAULT_NA_TOKENS,
            "NULL",
            true,
            false,
            true,
            true,
            "1,3",
            "minimal",
        )
        .unwrap();
        assert_eq!(got, "a,b,c\nNULL,NA,NULL\n");
    }

    #[test]
    fn tab_and_auto_delimiters_round_trip() {
        let tsv = "a\tb\n1\tNA\n";
        let by_name = standardize(
            tsv,
            "tab",
            DEFAULT_NA_TOKENS,
            "NULL",
            true,
            false,
            true,
            true,
            "",
            "minimal",
        )
        .unwrap();
        let by_sniff = standardize(
            tsv,
            "auto",
            DEFAULT_NA_TOKENS,
            "NULL",
            true,
            false,
            true,
            true,
            "",
            "minimal",
        )
        .unwrap();
        assert_eq!(by_name, "a\tb\n1\tNULL\n");
        assert_eq!(by_sniff, by_name);
    }

    #[test]
    fn semicolon_and_pipe_sniffing() {
        assert_eq!(
            standardize(
                "a;b\n1;NA\n",
                "auto",
                DEFAULT_NA_TOKENS,
                "NULL",
                true,
                false,
                true,
                true,
                "",
                "minimal"
            )
            .unwrap(),
            "a;b\n1;NULL\n"
        );
        assert_eq!(
            standardize(
                "a|b\n1|-\n",
                "auto",
                DEFAULT_NA_TOKENS,
                "NULL",
                true,
                false,
                true,
                true,
                "",
                "minimal"
            )
            .unwrap(),
            "a|b\n1|NULL\n"
        );
    }

    #[test]
    fn quote_style_always_quotes_every_field_including_the_replacement() {
        let got = standardize(
            "a,b\nNA,x\n",
            ",",
            DEFAULT_NA_TOKENS,
            "",
            true,
            false,
            true,
            true,
            "",
            "always",
        )
        .unwrap();
        // Note the missing cell is `""`, exactly like a real empty string would
        // be — `always` does NOT encode a null/empty-string distinction.
        assert_eq!(got, "\"a\",\"b\"\n\"\",\"x\"\n");
    }

    #[test]
    fn quote_style_never_writes_bare_fields() {
        let got = standardize(
            "a,b\nNA,\"x,y\"\n",
            ",",
            DEFAULT_NA_TOKENS,
            "NULL",
            true,
            false,
            true,
            true,
            "",
            "never",
        )
        .unwrap();
        assert_eq!(got, "a,b\nNULL,x,y\n");
    }

    #[test]
    fn quoted_fields_with_embedded_separators_and_newlines_survive() {
        let got = run("a,b\n\"x,y\",NA\n\"line1\nline2\",1\n", "NULL").unwrap();
        assert_eq!(got, "a,b\n\"x,y\",NULL\n\"line1\nline2\",1\n");
    }

    #[test]
    fn ragged_rows_keep_their_length() {
        let got = run("a,b,c\n1,NA\n2\n", "NULL").unwrap();
        assert_eq!(got, "a,b,c\n1,NULL\n2\n");
    }

    #[test]
    fn numeric_sentinels_can_be_added_as_tokens() {
        let got = standardize(
            "id,temp\n1,-999\n2,20\n",
            ",",
            "-999",
            "",
            true,
            false,
            true,
            true,
            "",
            "minimal",
        )
        .unwrap();
        assert_eq!(got, "id,temp\n1,\n2,20\n");
    }

    // --- errors ---

    #[test]
    fn empty_input_is_an_error() {
        let err = run("   \n", "NULL").unwrap_err();
        assert!(err.contains("input is empty"), "got: {err}");
    }

    #[test]
    fn unknown_column_name_is_an_error_listing_the_header() {
        let err = standardize(
            "a,b\n1,NA\n",
            ",",
            DEFAULT_NA_TOKENS,
            "NULL",
            true,
            false,
            true,
            true,
            "zzz",
            "minimal",
        )
        .unwrap_err();
        assert!(err.contains("'zzz' is not in the header"), "got: {err}");
        assert!(err.contains("available: a, b"), "got: {err}");
    }

    #[test]
    fn column_name_without_a_header_is_an_error() {
        let err = standardize(
            "1,NA\n",
            ",",
            DEFAULT_NA_TOKENS,
            "NULL",
            true,
            false,
            true,
            false,
            "score",
            "minimal",
        )
        .unwrap_err();
        assert!(err.contains("header option is off"), "got: {err}");
    }

    #[test]
    fn out_of_range_column_position_is_an_error() {
        let err = standardize(
            "a,b\n1,NA\n",
            ",",
            DEFAULT_NA_TOKENS,
            "NULL",
            true,
            false,
            true,
            true,
            "5",
            "minimal",
        )
        .unwrap_err();
        assert!(err.contains("out of range"), "got: {err}");
        assert!(err.contains("2 columns"), "got: {err}");
    }

    #[test]
    fn zero_column_position_is_an_error() {
        let err = standardize(
            "a,b\n1,NA\n",
            ",",
            DEFAULT_NA_TOKENS,
            "NULL",
            true,
            false,
            true,
            true,
            "0",
            "minimal",
        )
        .unwrap_err();
        assert!(err.contains("1-based"), "got: {err}");
    }

    #[test]
    fn bad_delimiter_and_quote_style_are_errors() {
        let e1 = standardize(
            "a,b\n1,NA\n",
            "commas",
            DEFAULT_NA_TOKENS,
            "NULL",
            true,
            false,
            true,
            true,
            "",
            "minimal",
        )
        .unwrap_err();
        assert!(e1.contains("delimiter must be"), "got: {e1}");
        let e2 = standardize(
            "a,b\n1,NA\n",
            ",",
            DEFAULT_NA_TOKENS,
            "NULL",
            true,
            false,
            true,
            true,
            "",
            "loud",
        )
        .unwrap_err();
        assert!(e2.contains("quote_style must be"), "got: {e2}");
    }

    #[test]
    fn input_size_cap_is_exact() {
        // One row of "a\n" repeated; at the cap it succeeds, one byte over it fails.
        let at_cap = "a\n".repeat(MAX_INPUT_BYTES / 2);
        assert_eq!(at_cap.len(), MAX_INPUT_BYTES);
        assert!(run(&at_cap, "NULL").is_ok());

        let over = format!("{at_cap}b");
        assert_eq!(over.len(), MAX_INPUT_BYTES + 1);
        let err = run(&over, "NULL").unwrap_err();
        assert!(err.contains("exceeds the"), "got: {err}");
    }
}
