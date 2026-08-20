//! gizza-ai/csv-ragged-row-padder core — pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps.
//!
//! Repairs a "ragged" CSV — one whose rows disagree about how many fields they
//! have — by normalizing every row to a single width: short rows are padded with
//! a fill value, and over-long rows are truncated, merged into the last column,
//! left alone but flagged, or dropped. Also strips a UTF-8 BOM, optionally drops
//! fully blank rows, re-quotes the output properly and normalizes line endings.
//! `output = "report"` returns a plain-text diagnostic instead of the CSV.

/// Largest input accepted, in bytes. Well above the interactive paste sizes the
/// page is for; scripted/huge files belong in the CLI.
pub const MAX_INPUT_BYTES: usize = 5_000_000;
/// Largest explicit target width accepted.
pub const MAX_WIDTH: usize = 10_000;

/// Resolve a delimiter spec to its byte. `"auto"`/empty returns `None`, meaning
/// "sniff it from the data".
fn delim_spec(d: &str) -> Result<Option<u8>, String> {
    Ok(match d.trim() {
        "" | "auto" => None,
        "," | "comma" => Some(b','),
        "\t" | "tab" | "\\t" => Some(b'\t'),
        ";" | "semicolon" => Some(b';'),
        "|" | "pipe" => Some(b'|'),
        other => {
            let b = other.as_bytes();
            if b.len() == 1 {
                Some(b[0])
            } else {
                return Err(format!(
                    "delimiter must be 'auto', a single char, or comma/tab/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// Human name for a delimiter byte, for the report.
fn delim_name(b: u8) -> String {
    match b {
        b',' => "comma".into(),
        b'\t' => "tab".into(),
        b';' => "semicolon".into(),
        b'|' => "pipe".into(),
        other => format!("'{}'", other as char),
    }
}

/// Sniff the delimiter: for each candidate, count how many times it occurs
/// OUTSIDE double quotes on each non-blank line, and pick the candidate whose
/// per-line count is both non-zero and most consistent (ties → highest count,
/// then comma-first candidate order).
fn sniff_delimiter(data: &str) -> u8 {
    const CANDIDATES: [u8; 4] = [b',', b';', b'\t', b'|'];
    let lines: Vec<&str> = data
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(20)
        .collect();
    if lines.is_empty() {
        return b',';
    }
    let mut best = (b',', 0usize, usize::MAX); // (delim, count, inconsistency)
    let mut first = true;
    for cand in CANDIDATES {
        let counts: Vec<usize> = lines
            .iter()
            .map(|l| count_outside_quotes(l, cand))
            .collect();
        let total: usize = counts.iter().sum();
        if total == 0 {
            continue;
        }
        let modal = counts.iter().copied().max().unwrap_or(0);
        let inconsistency = counts.iter().map(|c| modal.abs_diff(*c)).sum::<usize>();
        if first || inconsistency < best.2 || (inconsistency == best.2 && total > best.1) {
            best = (cand, total, inconsistency);
            first = false;
        }
    }
    best.0
}

/// Count occurrences of `needle` in `line` that sit outside double-quoted spans.
fn count_outside_quotes(line: &str, needle: u8) -> usize {
    let mut in_quotes = false;
    let mut n = 0;
    for b in line.bytes() {
        if b == b'"' {
            in_quotes = !in_quotes;
        } else if b == needle && !in_quotes {
            n += 1;
        }
    }
    n
}

/// What to do with a row that has MORE fields than the target width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LongRows {
    Truncate,
    Merge,
    Flag,
    Drop,
}

impl LongRows {
    fn parse(s: &str) -> Result<Self, String> {
        Ok(match s.trim() {
            "" | "truncate" => LongRows::Truncate,
            "merge" => LongRows::Merge,
            "flag" => LongRows::Flag,
            "drop" => LongRows::Drop,
            other => {
                return Err(format!(
                    "long_rows must be truncate/merge/flag/drop, got '{other}'"
                ))
            }
        })
    }
}

/// Where the target width comes from when `width` is 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WidthFrom {
    Header,
    Max,
    Mode,
}

impl WidthFrom {
    fn parse(s: &str) -> Result<Self, String> {
        Ok(match s.trim() {
            "" | "header" => WidthFrom::Header,
            "max" => WidthFrom::Max,
            "mode" => WidthFrom::Mode,
            other => return Err(format!("width_from must be header/max/mode, got '{other}'")),
        })
    }
}

/// One repaired row, recorded for the report.
struct Change {
    line: usize,
    from: usize,
    what: &'static str,
}

/// Repair a ragged CSV to a uniform field count.
///
/// * `data` — the CSV text.
/// * `width` — explicit target field count; `0` infers it from `width_from`.
/// * `width_from` — `header` (first row), `max` (widest row) or `mode` (most
///   common row width); used only when `width == 0`.
/// * `long_rows` — `truncate` (drop the extras), `merge` (join the extras into
///   the last column, delimiter-separated), `flag` (leave the row at its own
///   width and list it) or `drop` (remove the row and list it).
/// * `pad_value` — what to append to short rows (default: an empty field).
/// * `header` — treat the first row as a header: a short header row is padded
///   with generated `column_N` names instead of `pad_value` (blank column names
///   break most importers).
/// * `delimiter` — `auto`, a single char, or comma/tab/semicolon/pipe. The output
///   uses the same delimiter as the input.
/// * `drop_empty_rows` — drop rows whose cells are all blank/whitespace.
/// * `line_ending` — `lf` or `crlf` for the output.
/// * `output` — `csv` (the repaired CSV) or `report` (a plain-text diagnostic).
#[allow(clippy::too_many_arguments)]
pub fn pad_ragged(
    data: &str,
    width: usize,
    width_from: &str,
    long_rows: &str,
    pad_value: &str,
    header: bool,
    delimiter: &str,
    drop_empty_rows: bool,
    line_ending: &str,
    output: &str,
) -> Result<String, String> {
    if data.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes, which is over the {MAX_INPUT_BYTES}-byte limit — use the gizza CLI for files this large",
            data.len()
        ));
    }
    if width > MAX_WIDTH {
        return Err(format!("width must be {MAX_WIDTH} or less, got {width}"));
    }
    let long = LongRows::parse(long_rows)?;
    let from = WidthFrom::parse(width_from)?;
    let newline = match line_ending.trim() {
        "" | "lf" => "\n",
        "crlf" => "\r\n",
        other => return Err(format!("line_ending must be 'lf' or 'crlf', got '{other}'")),
    };
    let want_report = match output.trim() {
        "" | "csv" => false,
        "report" => true,
        other => return Err(format!("output must be 'csv' or 'report', got '{other}'")),
    };

    // A BOM would otherwise become part of the first header name.
    let data = data.strip_prefix('\u{feff}').unwrap_or(data);
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }

    let (delim, detected) = match delim_spec(delimiter)? {
        Some(b) => (b, false),
        None => (sniff_delimiter(data), true),
    };

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let mut rows: Vec<(usize, Vec<String>)> = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("CSV parse error: {e}"))?;
        let line = rec.position().map(|p| p.line() as usize).unwrap_or(0);
        rows.push((line, rec.iter().map(|c| c.to_string()).collect()));
    }
    if rows.is_empty() {
        return Err("input has no rows".into());
    }

    // Fully-blank rows are dropped before the width is measured, so a stray
    // empty line can't drag `mode`/`max` around.
    let mut blank_dropped = 0usize;
    if drop_empty_rows {
        let before = rows.len();
        rows.retain(|(_, r)| !r.iter().all(|c| c.trim().is_empty()));
        blank_dropped = before - rows.len();
        if rows.is_empty() {
            return Err("every row was blank".into());
        }
    }

    let target = if width > 0 {
        width
    } else {
        match from {
            WidthFrom::Header => rows[0].1.len(),
            WidthFrom::Max => rows.iter().map(|(_, r)| r.len()).max().unwrap_or(0),
            WidthFrom::Mode => modal_width(&rows),
        }
    };
    if target == 0 {
        return Err("target width resolved to 0 fields".into());
    }

    let delim_str = (delim as char).to_string();
    let mut changes: Vec<Change> = Vec::new();
    let mut out_rows: Vec<Vec<String>> = Vec::with_capacity(rows.len());
    let (mut padded, mut truncated, mut merged, mut flagged, mut dropped) = (0, 0, 0, 0, 0);
    let mut unchanged = 0usize;

    for (idx, (line, mut row)) in rows.into_iter().enumerate() {
        let orig = row.len();
        if orig == target {
            unchanged += 1;
        } else if orig < target {
            if header && idx == 0 {
                // A blank column NAME breaks most importers, so a short header
                // gets generated names rather than the data pad value.
                while row.len() < target {
                    row.push(format!("column_{}", row.len() + 1));
                }
            } else {
                row.resize(target, pad_value.to_string());
            }
            padded += 1;
            changes.push(Change {
                line,
                from: orig,
                what: "padded",
            });
        } else if orig > target {
            match long {
                LongRows::Truncate => {
                    row.truncate(target);
                    truncated += 1;
                    changes.push(Change {
                        line,
                        from: orig,
                        what: "truncated",
                    });
                }
                LongRows::Merge => {
                    let tail = row.split_off(target - 1).join(&delim_str);
                    row.push(tail);
                    merged += 1;
                    changes.push(Change {
                        line,
                        from: orig,
                        what: "merged",
                    });
                }
                LongRows::Flag => {
                    flagged += 1;
                    changes.push(Change {
                        line,
                        from: orig,
                        what: "flagged",
                    });
                }
                LongRows::Drop => {
                    dropped += 1;
                    changes.push(Change {
                        line,
                        from: orig,
                        what: "dropped",
                    });
                    continue;
                }
            }
        }
        out_rows.push(row);
    }

    if want_report {
        let mut r = String::new();
        r.push_str(&format!(
            "Target width: {target} fields ({})\n",
            if width > 0 {
                "explicit".to_string()
            } else {
                match from {
                    WidthFrom::Header => "from the first row".to_string(),
                    WidthFrom::Max => "widest row".to_string(),
                    WidthFrom::Mode => "most common row width".to_string(),
                }
            }
        ));
        r.push_str(&format!(
            "Delimiter: {}{}\n",
            delim_name(delim),
            if detected { " (auto-detected)" } else { "" }
        ));
        r.push_str(&format!(
            "Rows kept: {} ({})\n",
            out_rows.len(),
            if header { "header + data" } else { "data only" }
        ));
        r.push_str(&format!("Rows already at the target width: {unchanged}\n"));
        r.push_str(&format!("Short rows padded: {padded}\n"));
        r.push_str(&format!("Long rows truncated: {truncated}\n"));
        r.push_str(&format!("Long rows merged: {merged}\n"));
        r.push_str(&format!("Long rows flagged: {flagged}\n"));
        r.push_str(&format!("Long rows dropped: {dropped}\n"));
        r.push_str(&format!("Blank rows dropped: {blank_dropped}\n"));
        if changes.is_empty() {
            r.push_str("\nNo ragged rows found — every row already had the target width.\n");
        } else {
            r.push('\n');
            for c in &changes {
                let action = match c.what {
                    "flagged" => format!("flagged, left at {} fields", c.from),
                    "dropped" => "dropped".to_string(),
                    other => format!("{other} to {target}"),
                };
                r.push_str(&format!("line {}: {} fields -> {action}\n", c.line, c.from));
            }
        }
        return Ok(r);
    }

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .terminator(csv::Terminator::Any(b'\n'))
        .from_writer(Vec::new());
    for row in &out_rows {
        wtr.write_record(row)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    let text = String::from_utf8(bytes).map_err(|e| format!("CSV write error: {e}"))?;
    Ok(if newline == "\n" {
        text
    } else {
        text.replace('\n', "\r\n")
    })
}

/// The most common row width (ties → the wider one).
fn modal_width(rows: &[(usize, Vec<String>)]) -> usize {
    let mut counts: Vec<(usize, usize)> = Vec::new();
    for (_, r) in rows {
        match counts.iter_mut().find(|(w, _)| *w == r.len()) {
            Some((_, n)) => *n += 1,
            None => counts.push((r.len(), 1)),
        }
    }
    counts
        .into_iter()
        .max_by_key(|(w, n)| (*n, *w))
        .map(|(w, _)| w)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(data: &str) -> Result<String, String> {
        pad_ragged(
            data, 0, "header", "truncate", "", true, ",", true, "lf", "csv",
        )
    }

    #[test]
    fn pads_short_rows_and_truncates_long_ones() {
        let out = pad("a,b,c\n1,2\n3,4,5,6\n7,8,9\n").unwrap();
        assert_eq!(out, "a,b,c\n1,2,\n3,4,5\n7,8,9\n");
    }

    #[test]
    fn merge_joins_extras_into_the_last_column() {
        let out = pad_ragged(
            "a,b,c\n1,2,3,4,5\n",
            0,
            "header",
            "merge",
            "",
            true,
            ",",
            true,
            "lf",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "a,b,c\n1,2,\"3,4,5\"\n");
    }

    #[test]
    fn flag_keeps_the_row_at_its_own_width() {
        let out = pad_ragged(
            "a,b\n1,2,3\n",
            0,
            "header",
            "flag",
            "",
            true,
            ",",
            true,
            "lf",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "a,b\n1,2,3\n");
    }

    #[test]
    fn drop_removes_over_long_rows() {
        let out = pad_ragged(
            "a,b\n1,2\n3,4,5\n",
            0,
            "header",
            "drop",
            "",
            true,
            ",",
            true,
            "lf",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "a,b\n1,2\n");
    }

    #[test]
    fn explicit_width_overrides_the_header() {
        let out = pad_ragged(
            "a,b\n1,2\n",
            4,
            "header",
            "truncate",
            "",
            true,
            ",",
            true,
            "lf",
            "csv",
        )
        .unwrap();
        // The header gains generated names; the data row gains empty cells.
        assert_eq!(out, "a,b,column_3,column_4\n1,2,,\n");
    }

    #[test]
    fn without_a_header_the_first_row_is_padded_like_any_data_row() {
        let out = pad_ragged(
            "a,b\n1,2\n",
            4,
            "header",
            "truncate",
            "",
            false,
            ",",
            true,
            "lf",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "a,b,,\n1,2,,\n");
    }

    #[test]
    fn width_from_max_and_mode_differ() {
        let data = "1,2\n3,4\n5,6,7,8\n";
        let max = pad_ragged(
            data, 0, "max", "truncate", "", false, ",", true, "lf", "csv",
        )
        .unwrap();
        assert_eq!(max, "1,2,,\n3,4,,\n5,6,7,8\n");
        let mode = pad_ragged(
            data, 0, "mode", "truncate", "", false, ",", true, "lf", "csv",
        )
        .unwrap();
        assert_eq!(mode, "1,2\n3,4\n5,6\n");
    }

    #[test]
    fn pad_value_fills_short_rows() {
        let out = pad_ragged(
            "a,b,c\n1\n",
            0,
            "header",
            "truncate",
            "NULL",
            true,
            ",",
            true,
            "lf",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "a,b,c\n1,NULL,NULL\n");
    }

    #[test]
    fn detects_a_semicolon_delimiter() {
        let out = pad_ragged(
            "a;b;c\n1;2\n",
            0,
            "header",
            "truncate",
            "",
            true,
            "auto",
            true,
            "lf",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "a;b;c\n1;2;\n");
    }

    #[test]
    fn quoted_fields_survive_and_are_requoted() {
        let out = pad("a,b,c\n\"x,y\",2\n").unwrap();
        assert_eq!(out, "a,b,c\n\"x,y\",2,\n");
    }

    #[test]
    fn drops_blank_rows_before_measuring() {
        let out = pad("a,b\n\n1,2\n").unwrap();
        assert_eq!(out, "a,b\n1,2\n");
    }

    #[test]
    fn strips_a_bom() {
        let out = pad("\u{feff}a,b\n1,2\n").unwrap();
        assert_eq!(out, "a,b\n1,2\n");
    }

    #[test]
    fn crlf_line_endings() {
        let out = pad_ragged(
            "a,b\n1,2\n",
            0,
            "header",
            "truncate",
            "",
            true,
            ",",
            true,
            "crlf",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "a,b\r\n1,2\r\n");
    }

    #[test]
    fn report_lists_every_repaired_line() {
        let out = pad_ragged(
            "a,b,c\n1,2\n3,4,5,6\n",
            0,
            "header",
            "truncate",
            "",
            true,
            ",",
            true,
            "lf",
            "report",
        )
        .unwrap();
        assert!(out.contains("Target width: 3 fields (from the first row)"));
        assert!(out.contains("Short rows padded: 1"));
        assert!(out.contains("Long rows truncated: 1"));
        assert!(out.contains("line 2: 2 fields -> padded to 3"));
        assert!(out.contains("line 3: 4 fields -> truncated to 3"));
    }

    #[test]
    fn report_says_so_when_nothing_is_ragged() {
        let out = pad_ragged(
            "a,b\n1,2\n",
            0,
            "header",
            "truncate",
            "",
            true,
            ",",
            true,
            "lf",
            "report",
        )
        .unwrap();
        assert!(out.contains("No ragged rows found"));
    }

    #[test]
    fn rejects_an_unknown_long_rows_mode() {
        let err = pad_ragged(
            "a,b\n1,2\n",
            0,
            "header",
            "explode",
            "",
            true,
            ",",
            true,
            "lf",
            "csv",
        )
        .unwrap_err();
        assert!(err.contains("long_rows must be"), "{err}");
    }

    #[test]
    fn rejects_empty_input() {
        assert!(pad("   ").unwrap_err().contains("input is empty"));
    }

    #[test]
    fn rejects_a_multi_char_delimiter() {
        let err = pad_ragged(
            "a,b\n1,2\n",
            0,
            "header",
            "truncate",
            "",
            true,
            "::",
            true,
            "lf",
            "csv",
        )
        .unwrap_err();
        assert!(err.contains("delimiter must be"), "{err}");
    }

    #[test]
    fn rejects_an_over_large_width() {
        let err = pad_ragged(
            "a,b\n1,2\n",
            MAX_WIDTH + 1,
            "header",
            "truncate",
            "",
            true,
            ",",
            true,
            "lf",
            "csv",
        )
        .unwrap_err();
        assert!(err.contains("width must be"), "{err}");
    }
}
