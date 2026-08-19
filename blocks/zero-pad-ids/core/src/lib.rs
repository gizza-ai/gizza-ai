//! zero-pad-ids core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps.
//!
//! Fixes the classic broken-identifier column: a spreadsheet or loader typed an
//! ID/code column as a number somewhere upstream, so `00042` came back as `42`
//! and the codes no longer sort, join, or match a fixed-width spec. This pads
//! (or strips) leading zeros on the chosen column(s) of a delimited table — or
//! of a plain one-per-line list, which is just a one-column table — to a fixed
//! width, and writes the table back with the same separator.
//!
//! Only the zeros change. Cells outside the selected columns, blank cells, and
//! (by default) cells that are not plain digits are copied through untouched;
//! real digits are never truncated to make a value fit.

use std::collections::HashSet;

/// Hard cap on the pasted table, so a runaway paste can't wedge the tab.
pub const MAX_INPUT_BYTES: usize = 5_000_000;

/// Largest target width we accept. Well past any real identifier, and small
/// enough that a typo can't ask for a megabyte of zeros per cell.
pub const MAX_WIDTH: i64 = 64;

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
/// non-empty line. Ties (and a line with none of them) fall back to a comma —
/// which is also what a plain one-value-per-line list wants.
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
    /// Quote every field — some readers treat a quoted field as text and stop
    /// re-eating the leading zeros.
    Always,
    /// Never quote. Compact, and can emit ambiguous CSV.
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

/// Which direction we are rewriting in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    /// Left-pad with zeros up to the target width.
    Pad,
    /// Remove every leading zero (width is not used).
    Strip,
}

fn parse_mode(s: &str) -> Result<Mode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "pad" => Ok(Mode::Pad),
        "strip" => Ok(Mode::Strip),
        other => Err(format!("mode must be 'pad' or 'strip', got '{other}'")),
    }
}

/// What to do with a value that is already at or over the target width.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Overflow {
    /// Leave it exactly as it is (default).
    Keep,
    /// Drop excess leading zeros so it lands on the width when possible.
    Strip,
    /// Fail, naming the row, column, and value.
    Error,
}

fn parse_overflow(s: &str) -> Result<Overflow, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "keep" => Ok(Overflow::Keep),
        "strip" => Ok(Overflow::Strip),
        "error" => Ok(Overflow::Error),
        other => Err(format!(
            "overflow must be 'keep', 'strip', or 'error', got '{other}'"
        )),
    }
}

/// What to do with a cell that is not a plain run of digits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NonNumeric {
    /// Copy it through untouched (default).
    Keep,
    /// Pad it anyway — for alphanumeric codes like `AB12`.
    Pad,
    /// Fail, naming the row, column, and value.
    Error,
}

fn parse_non_numeric(s: &str) -> Result<NonNumeric, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "keep" => Ok(NonNumeric::Keep),
        "pad" => Ok(NonNumeric::Pad),
        "error" => Ok(NonNumeric::Error),
        other => Err(format!(
            "non_numeric must be 'keep', 'pad', or 'error', got '{other}'"
        )),
    }
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
                    h.iter().map(|c| c.trim()).collect::<Vec<_>>().join(", ")
                ))
            }
        }
    }
    if out.is_empty() {
        return Err("columns selector resolved to no columns".to_string());
    }
    Ok(Some(out))
}

/// A plain run of ASCII digits — what an ID column normally holds. `-42`,
/// `1.5`, `1e3` and `SKU-9` all fail this, so they follow the `non_numeric`
/// policy instead of being silently reformatted.
fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// How this cell should be reported in an error message.
fn col_label(header_row: Option<&[String]>, idx: usize) -> String {
    match header_row.and_then(|h| h.get(idx)) {
        Some(name) if !name.trim().is_empty() => format!("column '{}'", name.trim()),
        _ => format!("column {}", idx + 1),
    }
}

/// Drop leading zeros, keeping at least one character. `"000"` → `"0"`,
/// `"00042"` → `"42"`, `"0"` → `"0"`.
fn strip_zeros(s: &str) -> String {
    let trimmed = s.trim_start_matches('0');
    if trimmed.is_empty() {
        // The value was all zeros; a bare "0" is the honest result.
        s.chars().next().map(|c| c.to_string()).unwrap_or_default()
    } else {
        trimmed.to_string()
    }
}

/// Pad (or strip) leading zeros on an ID/code column of a delimited table.
///
/// * `data` — the table text (a plain one-value-per-line list is a one-column table).
/// * `delimiter` — `auto`, a single character, or `comma`/`tab`/`semicolon`/`pipe`.
/// * `columns` — comma-separated column names (needs a header) or 1-based
///   positions to rewrite; empty = every column.
/// * `width` — target width in characters. `0` means auto: each selected column
///   is padded up to its own widest eligible value. Ignored when `mode` is `strip`.
/// * `mode` — `pad` (default) or `strip`.
/// * `overflow` — `keep` (default), `strip`, or `error`, for values already at
///   or over `width`. Pad mode only.
/// * `non_numeric` — `keep` (default), `pad`, or `error`, for cells that are not
///   a plain run of digits. Blank cells are always left blank.
/// * `header` — treat the first row as a header: never rewritten, and it supplies
///   the names used by `columns`.
/// * `quote_style` — `minimal` (default), `always`, or `never`.
#[allow(clippy::too_many_arguments)]
pub fn zero_pad(
    data: &str,
    delimiter: &str,
    columns: &str,
    width: i64,
    mode: &str,
    overflow: &str,
    non_numeric: &str,
    header: bool,
    quote_style: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err(
            "input is empty — paste a table or a one-per-line list of IDs with at least one row"
                .to_string(),
        );
    }
    if data.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes, which exceeds the {MAX_INPUT_BYTES}-byte limit",
            data.len()
        ));
    }
    if !(0..=MAX_WIDTH).contains(&width) {
        return Err(format!(
            "width must be between 0 and {MAX_WIDTH} (0 = auto-fit to the widest value), got {width}"
        ));
    }

    let delim = delim_byte(delimiter, data)?;
    let quoting = parse_quoting(quote_style)?;
    let mode = parse_mode(mode)?;
    let overflow = parse_overflow(overflow)?;
    let non_numeric = parse_non_numeric(non_numeric)?;

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
        return Err(
            "input is empty — paste a table or a one-per-line list of IDs with at least one row"
                .to_string(),
        );
    }

    let header_row = if header { Some(rows[0].clone()) } else { None };
    let selected = parse_columns(columns, header_row.as_deref())?;
    let first_data_row = if header { 1 } else { 0 };

    // Auto width (`width = 0`): each selected column gets its own target, the
    // longest eligible value in that column. Ragged rows are fine — a column
    // with no eligible value keeps a target of 0 and is left alone.
    let mut auto_width: Vec<usize> = Vec::new();
    if mode == Mode::Pad && width == 0 {
        for row in rows.iter().skip(first_data_row) {
            for (c, cell) in row.iter().enumerate() {
                let t = cell.trim();
                if t.is_empty() {
                    continue;
                }
                let in_scope = selected.as_ref().is_none_or(|s| s.contains(&c));
                if !in_scope {
                    continue;
                }
                let eligible = all_digits(t) || non_numeric == NonNumeric::Pad;
                if !eligible {
                    continue;
                }
                let len = t.chars().count();
                if auto_width.len() <= c {
                    auto_width.resize(c + 1, 0);
                }
                if len > auto_width[c] {
                    auto_width[c] = len;
                }
            }
        }
    }

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
        let mut out: Vec<String> = Vec::with_capacity(row.len());
        for (c, cell) in row.iter().enumerate() {
            let in_scope = selected.as_ref().is_none_or(|s| s.contains(&c));
            let t = cell.trim();
            // Out of scope, or nothing there: copied byte-for-byte. A blank cell
            // is never invented into "00000".
            if !in_scope || t.is_empty() {
                out.push(cell.clone());
                continue;
            }
            if !all_digits(t) {
                match non_numeric {
                    NonNumeric::Keep => {
                        out.push(cell.clone());
                        continue;
                    }
                    NonNumeric::Error => {
                        return Err(format!(
                            "row {}, {}: expected a plain run of digits, got '{}' — set non_numeric to 'keep' to copy it through or 'pad' to pad it anyway",
                            r + 1,
                            col_label(header_row.as_deref(), c),
                            t
                        ))
                    }
                    NonNumeric::Pad => {}
                }
            }

            let new = match mode {
                Mode::Strip => strip_zeros(t),
                Mode::Pad => {
                    let target = if width > 0 {
                        width as usize
                    } else {
                        auto_width.get(c).copied().unwrap_or(0)
                    };
                    let len = t.chars().count();
                    if len < target {
                        let mut s = String::with_capacity(target);
                        for _ in 0..(target - len) {
                            s.push('0');
                        }
                        s.push_str(t);
                        s
                    } else {
                        match overflow {
                            Overflow::Keep => t.to_string(),
                            Overflow::Strip => {
                                // Only zeros are ever removed — real digits are
                                // never truncated, so a genuinely wider value
                                // survives intact.
                                let stripped = strip_zeros(t);
                                let slen = stripped.chars().count();
                                if slen < target {
                                    let mut s = String::with_capacity(target);
                                    for _ in 0..(target - slen) {
                                        s.push('0');
                                    }
                                    s.push_str(&stripped);
                                    s
                                } else {
                                    stripped
                                }
                            }
                            Overflow::Error => {
                                return Err(format!(
                                    "row {}, {}: '{}' is {} characters, at or over the target width of {} — set overflow to 'keep' to leave it or 'strip' to drop its leading zeros",
                                    r + 1,
                                    col_label(header_row.as_deref(), c),
                                    t,
                                    len,
                                    target
                                ))
                            }
                        }
                    }
                }
            };
            out.push(new);
        }
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

    /// Defaults everywhere except the two knobs a test actually cares about.
    fn pad(data: &str, width: i64, columns: &str) -> Result<String, String> {
        zero_pad(
            data, ",", columns, width, "pad", "keep", "keep", true, "minimal",
        )
    }

    #[test]
    fn pads_a_named_column_to_a_fixed_width() {
        let got = pad("id,name\n42,ada\n7,linus\n12345,grace\n", 5, "id").unwrap();
        assert_eq!(got, "id,name\n00042,ada\n00007,linus\n12345,grace\n");
    }

    #[test]
    fn pads_a_plain_one_per_line_list() {
        // No delimiter in sight: a one-column table, header off.
        let got = zero_pad(
            "42\n7\n1234\n",
            "auto",
            "",
            6,
            "pad",
            "keep",
            "keep",
            false,
            "minimal",
        )
        .unwrap();
        assert_eq!(got, "000042\n000007\n001234\n");
    }

    #[test]
    fn auto_width_fits_each_column_to_its_own_widest_value() {
        let got = pad("sku,qty\n7,3\n1234,10\n", 0, "").unwrap();
        assert_eq!(got, "sku,qty\n0007,03\n1234,10\n");
    }

    #[test]
    fn header_row_is_never_rewritten() {
        // A header cell that is itself all digits survives.
        let got = pad("2024,name\n42,ada\n", 5, "1").unwrap();
        assert_eq!(got, "2024,name\n00042,ada\n");
    }

    #[test]
    fn header_off_rewrites_the_first_row_too() {
        let got = zero_pad(
            "42,ada\n7,linus\n",
            ",",
            "1",
            4,
            "pad",
            "keep",
            "keep",
            false,
            "minimal",
        )
        .unwrap();
        assert_eq!(got, "0042,ada\n0007,linus\n");
    }

    #[test]
    fn columns_accepts_one_based_positions() {
        let got = pad("a,b\n1,2\n", 3, "2").unwrap();
        assert_eq!(got, "a,b\n1,002\n");
    }

    #[test]
    fn blank_cells_stay_blank() {
        let got = pad("id,name\n,ada\n7,linus\n", 4, "id").unwrap();
        assert_eq!(got, "id,name\n,ada\n0007,linus\n");
    }

    #[test]
    fn non_numeric_cells_are_kept_verbatim_by_default() {
        let got = pad("id\nSKU-9\n42\nN/A\n", 5, "").unwrap();
        assert_eq!(got, "id\nSKU-9\n00042\nN/A\n");
    }

    #[test]
    fn non_numeric_pad_covers_alphanumeric_codes() {
        let got = zero_pad(
            "id\nAB12\n42\n",
            ",",
            "",
            6,
            "pad",
            "keep",
            "pad",
            true,
            "minimal",
        )
        .unwrap();
        assert_eq!(got, "id\n00AB12\n000042\n");
    }

    #[test]
    fn non_numeric_error_names_the_row_and_column() {
        let err = zero_pad(
            "id\nSKU-9\n",
            ",",
            "",
            5,
            "pad",
            "keep",
            "error",
            true,
            "minimal",
        )
        .unwrap_err();
        assert!(err.contains("row 2"), "{err}");
        assert!(err.contains("column 'id'"), "{err}");
        assert!(err.contains("SKU-9"), "{err}");
    }

    #[test]
    fn overflow_keep_leaves_wider_values_alone() {
        let got = pad("id\n1234567\n42\n", 5, "").unwrap();
        assert_eq!(got, "id\n1234567\n00042\n");
    }

    #[test]
    fn overflow_strip_renormalizes_over_padded_values() {
        let got = zero_pad(
            "id\n0000012\n00012345\n123456\n",
            ",",
            "",
            5,
            "pad",
            "strip",
            "keep",
            true,
            "minimal",
        )
        .unwrap();
        // 0000012 → 00012 (re-padded to 5); 00012345 → 12345 (exactly 5);
        // 123456 has no leading zeros, so real digits are never truncated.
        assert_eq!(got, "id\n00012\n12345\n123456\n");
    }

    #[test]
    fn overflow_error_reports_the_offending_value() {
        let err = zero_pad(
            "id\n1234567\n",
            ",",
            "",
            5,
            "pad",
            "error",
            "keep",
            true,
            "minimal",
        )
        .unwrap_err();
        assert!(err.contains("row 2"), "{err}");
        assert!(err.contains("1234567"), "{err}");
        assert!(err.contains("width of 5"), "{err}");
    }

    #[test]
    fn strip_mode_removes_every_leading_zero() {
        let got = zero_pad(
            "id\n00042\n000\n7\n",
            ",",
            "",
            0,
            "strip",
            "keep",
            "keep",
            true,
            "minimal",
        )
        .unwrap();
        assert_eq!(got, "id\n42\n0\n7\n");
    }

    #[test]
    fn tab_delimited_input_round_trips_its_separator() {
        let got = zero_pad(
            "id\tname\n42\tada\n",
            "auto",
            "id",
            4,
            "pad",
            "keep",
            "keep",
            true,
            "minimal",
        )
        .unwrap();
        assert_eq!(got, "id\tname\n0042\tada\n");
    }

    #[test]
    fn quoted_fields_and_ragged_rows_survive() {
        let got = pad("id,note\n42,\"a,b\"\n7\n", 4, "id").unwrap();
        assert_eq!(got, "id,note\n0042,\"a,b\"\n0007\n");
    }

    #[test]
    fn quote_style_always_quotes_every_field() {
        let got = zero_pad(
            "id\n42\n", ",", "", 4, "pad", "keep", "keep", true, "always",
        )
        .unwrap();
        assert_eq!(got, "\"id\"\n\"0042\"\n");
    }

    #[test]
    fn out_of_scope_columns_are_copied_byte_for_byte() {
        // The padded column loses its surrounding whitespace; an untouched one
        // keeps it.
        let got = pad("id,qty\n  42  ,  9  \n", 4, "id").unwrap();
        assert_eq!(got, "id,qty\n0042,  9  \n");
    }

    #[test]
    fn empty_input_is_rejected() {
        let err = pad("   \n", 5, "").unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn unknown_column_name_lists_the_available_ones() {
        let err = pad("id,name\n42,ada\n", 5, "sku").unwrap_err();
        assert!(err.contains("'sku' is not in the header"), "{err}");
        assert!(err.contains("id, name"), "{err}");
    }

    #[test]
    fn column_name_without_a_header_is_rejected() {
        let err = zero_pad(
            "42,ada\n", ",", "id", 5, "pad", "keep", "keep", false, "minimal",
        )
        .unwrap_err();
        assert!(err.contains("header option is off"), "{err}");
    }

    #[test]
    fn out_of_range_width_is_rejected() {
        let err = pad("id\n42\n", 999, "").unwrap_err();
        assert!(err.contains("width must be between 0 and 64"), "{err}");
    }

    #[test]
    fn bad_enum_values_are_rejected_with_the_allowed_set() {
        let err = zero_pad(
            "id\n42\n", ",", "", 5, "sideways", "keep", "keep", true, "minimal",
        )
        .unwrap_err();
        assert!(err.contains("mode must be 'pad' or 'strip'"), "{err}");
        let err = zero_pad(
            "id\n42\n", ",", "", 5, "pad", "explode", "keep", true, "minimal",
        )
        .unwrap_err();
        assert!(err.contains("overflow must be"), "{err}");
        let err = zero_pad(
            "id\n42\n", ",", "", 5, "pad", "keep", "maybe", true, "minimal",
        )
        .unwrap_err();
        assert!(err.contains("non_numeric must be"), "{err}");
    }

    #[test]
    fn multi_character_delimiter_is_rejected() {
        let err = zero_pad(
            "id\n42\n", "::", "", 5, "pad", "keep", "keep", true, "minimal",
        )
        .unwrap_err();
        assert!(err.contains("delimiter must be"), "{err}");
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = format!("id\n{}\n", "1".repeat(MAX_INPUT_BYTES));
        let err = pad(&big, 5, "").unwrap_err();
        assert!(err.contains("exceeds the"), "{err}");
    }
}
