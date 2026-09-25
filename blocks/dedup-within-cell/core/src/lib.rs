//! dedup-within-cell core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps. Removes duplicate ITEMS INSIDE each
//! delimited cell of a CSV/TSV table (`"a, b, a, c"` → `"a, b, c"`), keeping the
//! first occurrence, optionally sorting what survives.
//!
//! This is the within-cell counterpart to row deduplication (`csv-dedupe`,
//! `csv-cleaner`): rows and columns are preserved exactly; only the list packed
//! inside a cell is cleaned up. A single-column input (one list per line) works
//! too — that's just a one-column table.

use std::collections::HashSet;

/// Hard cap on input size (bytes). The whole table is parsed in memory by a
/// synchronous pure function; 1 MB keeps it instant.
pub const MAX_INPUT_BYTES: usize = 1_000_000;

/// Resolve a single-character CSV field delimiter from a name or literal.
fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            return Err(format!(
                "delimiter must be comma/tab/semicolon/pipe, got '{other}'"
            ))
        }
    })
}

/// Resolve the separator BETWEEN ITEMS inside one cell. Accepts a friendly name
/// or any literal string (`", "`, `" | "`, `" - "`…). Empty → comma.
fn item_sep(spec: &str) -> Result<String, String> {
    let s = match spec {
        "" | "comma" => ",".to_string(),
        "semicolon" => ";".to_string(),
        "pipe" => "|".to_string(),
        "space" => " ".to_string(),
        "tab" | "\\t" => "\t".to_string(),
        "newline" | "\\n" => "\n".to_string(),
        "comma-space" => ", ".to_string(),
        other => other.to_string(),
    };
    if s.is_empty() {
        return Err("item_separator must not be empty".into());
    }
    Ok(s)
}

/// How the surviving items are ordered.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sort {
    /// Keep the order items first appear in the cell (default).
    FirstSeen,
    Asc,
    Desc,
}

impl Sort {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "none" | "first-seen" => Ok(Sort::FirstSeen),
            "asc" => Ok(Sort::Asc),
            "desc" => Ok(Sort::Desc),
            other => Err(format!("sort_items must be none/asc/desc, got '{other}'")),
        }
    }
}

/// What the tool returns.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OutputKind {
    /// The rewritten CSV (default).
    Csv,
    /// A short plain-text report of what was removed.
    Stats,
}

impl OutputKind {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "csv" => Ok(OutputKind::Csv),
            "stats" => Ok(OutputKind::Stats),
            other => Err(format!("output must be csv/stats, got '{other}'")),
        }
    }
}

/// Resolve `columns` (1-based indices and/or header names) to 0-based indices.
/// Empty → `None`, meaning every column is processed.
fn resolve_columns(
    columns: &str,
    header: Option<&csv::StringRecord>,
) -> Result<Option<Vec<usize>>, String> {
    let toks: Vec<&str> = columns
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if toks.is_empty() {
        return Ok(None);
    }
    let mut idxs = Vec::new();
    for t in toks {
        if let Ok(n) = t.parse::<usize>() {
            if n == 0 {
                return Err("column indices are 1-based (>= 1)".into());
            }
            idxs.push(n - 1);
        } else if let Some(h) = header {
            match h.iter().position(|c| c == t) {
                Some(p) => idxs.push(p),
                None => return Err(format!("column '{t}' not found in the header")),
            }
        } else {
            return Err(format!(
                "column '{t}' is a name but there is no header row — use 1-based indices when has_header is off"
            ));
        }
    }
    Ok(Some(idxs))
}

/// The join string used when `output_separator` is blank: the cell's OWN first
/// separator plus whatever whitespace followed it, so `"a, b, a"` stays
/// comma-space and `"a;b;a"` stays bare-semicolon.
fn inferred_join(cell: &str, sep: &str) -> String {
    match cell.find(sep) {
        Some(at) => {
            let rest = &cell[at + sep.len()..];
            let ws: String = rest
                .chars()
                .take_while(|c| c.is_whitespace() && *c != '\n' && *c != '\r')
                .collect();
            format!("{sep}{ws}")
        }
        None => sep.to_string(),
    }
}

/// Dedupe one cell. Returns the rewritten cell plus how many duplicate items
/// were dropped.
fn dedupe_cell(
    cell: &str,
    sep: &str,
    join: &str,
    ignore_case: bool,
    trim_items: bool,
    drop_empty: bool,
    sort: Sort,
) -> (String, usize) {
    let mut kept: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut removed = 0usize;
    for raw in cell.split(sep) {
        let item = if trim_items { raw.trim() } else { raw };
        if drop_empty && item.trim().is_empty() {
            continue;
        }
        let key = if ignore_case {
            item.to_lowercase()
        } else {
            item.to_string()
        };
        if seen.insert(key) {
            kept.push(item.to_string());
        } else {
            removed += 1;
        }
    }
    let cmp_key = |s: &String| {
        if ignore_case {
            s.to_lowercase()
        } else {
            s.clone()
        }
    };
    match sort {
        Sort::FirstSeen => {}
        Sort::Asc => kept.sort_by_key(cmp_key),
        Sort::Desc => {
            kept.sort_by_key(cmp_key);
            kept.reverse();
        }
    }
    (kept.join(join), removed)
}

/// Remove duplicate items inside every (selected) cell of a CSV/TSV table.
///
/// * `data` — the CSV/TSV text.
/// * `columns` — comma-separated header names and/or 1-based indices to process;
///   empty processes every column.
/// * `item_separator` — separator between items INSIDE a cell: a name
///   (comma/semicolon/pipe/space/tab/newline/comma-space) or any literal string.
/// * `output_separator` — separator used to re-join the survivors; empty reuses
///   the cell's own separator+spacing.
/// * `ignore_case` — match items case-insensitively (the first occurrence's
///   casing is the one kept).
/// * `trim_items` — trim whitespace around each item before comparing (on by
///   default; also normalizes the output).
/// * `drop_empty` — drop empty items, so runs of separators collapse.
/// * `sort_items` — `none` (first-seen), `asc`, or `desc`.
/// * `delimiter` — CSV FIELD delimiter (comma/tab/semicolon/pipe), used for both
///   reading and writing.
/// * `has_header` — treat the first row as a header: never rewritten, and
///   columns may be named.
/// * `output` — `csv` (the rewritten table) or `stats` (a removal report).
#[allow(clippy::too_many_arguments)]
pub fn dedupe_within_cells(
    data: &str,
    columns: &str,
    item_separator: &str,
    output_separator: &str,
    ignore_case: bool,
    trim_items: bool,
    drop_empty: bool,
    sort_items: &str,
    delimiter: &str,
    has_header: bool,
    output: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("data is empty — paste CSV text (or one delimited list per line)".into());
    }
    if data.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the limit is {} bytes (1 MB)",
            data.len(),
            MAX_INPUT_BYTES
        ));
    }
    let delim = delim_byte(delimiter)?;
    let sep = item_sep(item_separator)?;
    let sort = Sort::parse(sort_items)?;
    let kind = OutputKind::parse(output)?;

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let mut records = Vec::new();
    for rec in rdr.records() {
        records.push(rec.map_err(|e| format!("could not parse the CSV: {e}"))?);
    }
    if records.is_empty() {
        return Err("no rows found in the input".into());
    }

    let header = if has_header {
        Some(records[0].clone())
    } else {
        None
    };
    let targets = resolve_columns(columns, header.as_ref())?;
    if let (Some(idxs), Some(h)) = (targets.as_ref(), header.as_ref()) {
        for &i in idxs {
            if i >= h.len() {
                return Err(format!(
                    "column {} is out of range — the header has {} column(s)",
                    i + 1,
                    h.len()
                ));
            }
        }
    }

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(vec![]);
    let mut cells_scanned = 0usize;
    let mut cells_changed = 0usize;
    let mut items_removed = 0usize;

    for (row_i, rec) in records.iter().enumerate() {
        if has_header && row_i == 0 {
            wtr.write_record(rec.iter())
                .map_err(|e| format!("could not write the CSV: {e}"))?;
            continue;
        }
        let mut out_row: Vec<String> = Vec::with_capacity(rec.len());
        for (col_i, cell) in rec.iter().enumerate() {
            let selected = match targets.as_ref() {
                None => true,
                Some(idxs) => idxs.contains(&col_i),
            };
            if !selected {
                out_row.push(cell.to_string());
                continue;
            }
            cells_scanned += 1;
            let join = if output_separator.is_empty() {
                inferred_join(cell, &sep)
            } else {
                item_sep(output_separator)?
            };
            let (new_cell, removed) =
                dedupe_cell(cell, &sep, &join, ignore_case, trim_items, drop_empty, sort);
            if new_cell != cell {
                cells_changed += 1;
            }
            items_removed += removed;
            out_row.push(new_cell);
        }
        wtr.write_record(&out_row)
            .map_err(|e| format!("could not write the CSV: {e}"))?;
    }

    let csv_out = String::from_utf8(
        wtr.into_inner()
            .map_err(|e| format!("could not finish the CSV: {e}"))?,
    )
    .map_err(|e| format!("output was not valid UTF-8: {e}"))?;

    Ok(match kind {
        OutputKind::Csv => csv_out,
        OutputKind::Stats => {
            let data_rows = records.len() - usize::from(has_header);
            format!(
                "data rows: {data_rows}\ncells scanned: {cells_scanned}\ncells changed: {cells_changed}\nduplicate items removed: {items_removed}\n"
            )
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Defaults: comma items, trim, drop empties, first-seen order, header kept.
    fn run(data: &str) -> String {
        dedupe_within_cells(
            data, "", "", "", false, true, true, "none", "comma", true, "csv",
        )
        .unwrap()
    }

    #[test]
    fn dedupes_items_inside_a_quoted_cell_and_keeps_spacing() {
        let out = run("id,tags\n1,\"a, b, a, c\"\n");
        assert_eq!(out, "id,tags\n1,\"a, b, c\"\n");
    }

    #[test]
    fn bare_list_without_header_is_a_one_column_table() {
        let out = dedupe_within_cells(
            "a;b;a;b;c",
            "",
            ";",
            "",
            false,
            true,
            true,
            "none",
            "comma",
            false,
            "csv",
        )
        .unwrap();
        assert_eq!(out, "a;b;c\n");
    }

    #[test]
    fn header_row_is_never_rewritten() {
        // The header itself contains a repeated comma-separated look-alike.
        let out = run("\"x, x\",tags\n1,\"a, a\"\n");
        assert_eq!(out, "\"x, x\",tags\n1,a\n");
    }

    #[test]
    fn only_selected_columns_are_touched() {
        let out = dedupe_within_cells(
            "tags,other\n\"a, a\",\"b, b\"\n",
            "tags",
            "",
            "",
            false,
            true,
            true,
            "none",
            "comma",
            true,
            "csv",
        )
        .unwrap();
        assert_eq!(out, "tags,other\na,\"b, b\"\n");
    }

    #[test]
    fn columns_accept_one_based_indices() {
        let out = dedupe_within_cells(
            "tags,other\n\"a, a\",\"b, b\"\n",
            "2",
            "",
            "",
            false,
            true,
            true,
            "none",
            "comma",
            true,
            "csv",
        )
        .unwrap();
        assert_eq!(out, "tags,other\n\"a, a\",b\n");
    }

    #[test]
    fn ignore_case_keeps_the_first_casing() {
        let out = dedupe_within_cells(
            "tags\n\"Apple, apple, APPLE, Pear\"\n",
            "",
            "",
            "",
            true,
            true,
            true,
            "none",
            "comma",
            true,
            "csv",
        )
        .unwrap();
        assert_eq!(out, "tags\n\"Apple, Pear\"\n");
    }

    #[test]
    fn case_sensitive_by_default() {
        let out = run("tags\n\"Apple, apple\"\n");
        assert_eq!(out, "tags\n\"Apple, apple\"\n");
    }

    #[test]
    fn sort_asc_and_desc() {
        let asc = dedupe_within_cells(
            "tags\n\"c, a, b, a\"\n",
            "",
            "",
            "",
            false,
            true,
            true,
            "asc",
            "comma",
            true,
            "csv",
        )
        .unwrap();
        assert_eq!(asc, "tags\n\"a, b, c\"\n");
        let desc = dedupe_within_cells(
            "tags\n\"c, a, b, a\"\n",
            "",
            "",
            "",
            false,
            true,
            true,
            "desc",
            "comma",
            true,
            "csv",
        )
        .unwrap();
        assert_eq!(desc, "tags\n\"c, b, a\"\n");
    }

    #[test]
    fn sort_is_case_insensitive_when_ignore_case_is_on() {
        let out = dedupe_within_cells(
            "tags\n\"banana, Apple, cherry\"\n",
            "",
            "",
            "",
            true,
            true,
            true,
            "asc",
            "comma",
            true,
            "csv",
        )
        .unwrap();
        assert_eq!(out, "tags\n\"Apple, banana, cherry\"\n");
    }

    #[test]
    fn output_separator_normalizes_the_join() {
        let out = dedupe_within_cells(
            "tags\na;b;a\n",
            "",
            ";",
            ", ",
            false,
            true,
            true,
            "none",
            "comma",
            true,
            "csv",
        )
        .unwrap();
        assert_eq!(out, "tags\n\"a, b\"\n");
    }

    #[test]
    fn drop_empty_collapses_repeated_separators() {
        let out = run("tags\n\"a,,b,,,a\"\n");
        assert_eq!(out, "tags\n\"a,b\"\n");
    }

    #[test]
    fn keeping_empties_treats_blank_as_an_item() {
        let out = dedupe_within_cells(
            "tags\n\"a,,b,,a\"\n",
            "",
            ",",
            "",
            false,
            true,
            false,
            "none",
            "comma",
            true,
            "csv",
        )
        .unwrap();
        // The first blank survives as an item, later blanks are duplicates.
        assert_eq!(out, "tags\n\"a,,b\"\n");
    }

    #[test]
    fn untrimmed_items_compare_literally() {
        let out = dedupe_within_cells(
            "tags\n\"a, a,a\"\n",
            "",
            ",",
            ", ",
            false,
            false,
            true,
            "none",
            "comma",
            true,
            "csv",
        )
        .unwrap();
        // " a" and "a" differ without trimming, so only the third is a dup.
        assert_eq!(out, "tags\n\"a,  a\"\n");
    }

    #[test]
    fn tab_delimited_input_round_trips() {
        let out = dedupe_within_cells(
            "id\ttags\n1\ta, b, a\n",
            "",
            "",
            "",
            false,
            true,
            true,
            "none",
            "tab",
            true,
            "csv",
        )
        .unwrap();
        assert_eq!(out, "id\ttags\n1\ta, b\n");
    }

    #[test]
    fn newline_separated_items_inside_one_cell() {
        let out = dedupe_within_cells(
            "tags\n\"a\nb\na\"\n",
            "",
            "newline",
            "",
            false,
            true,
            true,
            "none",
            "comma",
            true,
            "csv",
        )
        .unwrap();
        assert_eq!(out, "tags\n\"a\nb\"\n");
    }

    #[test]
    fn stats_output_reports_what_changed() {
        let out = dedupe_within_cells(
            "id,tags\n1,\"a, b, a\"\n2,\"c, c, c\"\n",
            "tags",
            "",
            "",
            false,
            true,
            true,
            "none",
            "comma",
            true,
            "stats",
        )
        .unwrap();
        assert_eq!(
            out,
            "data rows: 2\ncells scanned: 2\ncells changed: 2\nduplicate items removed: 3\n"
        );
    }

    #[test]
    fn at_the_cap_it_still_runs() {
        let mut data = String::from("tags\n");
        let cell = "a, a, b\n";
        while data.len() + cell.len() <= MAX_INPUT_BYTES {
            data.push_str(cell);
        }
        assert!(data.len() <= MAX_INPUT_BYTES);
        let out = run(&data);
        assert!(out.starts_with("tags\n"));
        assert!(out.len() <= data.len());
    }

    #[test]
    fn one_byte_over_the_cap_is_rejected() {
        let data = "a".repeat(MAX_INPUT_BYTES + 1);
        let err = dedupe_within_cells(
            &data, "", "", "", false, true, true, "none", "comma", false, "csv",
        )
        .unwrap_err();
        assert!(err.contains("the limit is 1000000 bytes"), "{err}");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = dedupe_within_cells(
            "   ", "", "", "", false, true, true, "none", "comma", true, "csv",
        )
        .unwrap_err();
        assert!(err.contains("data is empty"), "{err}");
    }

    #[test]
    fn unknown_column_name_is_an_error() {
        let err = dedupe_within_cells(
            "id,tags\n1,a\n",
            "nope",
            "",
            "",
            false,
            true,
            true,
            "none",
            "comma",
            true,
            "csv",
        )
        .unwrap_err();
        assert_eq!(err, "column 'nope' not found in the header");
    }

    #[test]
    fn named_column_without_a_header_is_an_error() {
        let err = dedupe_within_cells(
            "a,b\n", "tags", "", "", false, true, true, "none", "comma", false, "csv",
        )
        .unwrap_err();
        assert!(err.contains("use 1-based indices"), "{err}");
    }

    #[test]
    fn out_of_range_column_index_is_an_error() {
        let err = dedupe_within_cells(
            "id,tags\n1,a\n",
            "5",
            "",
            "",
            false,
            true,
            true,
            "none",
            "comma",
            true,
            "csv",
        )
        .unwrap_err();
        assert_eq!(err, "column 5 is out of range — the header has 2 column(s)");
    }

    #[test]
    fn zero_index_is_rejected() {
        let err = dedupe_within_cells(
            "id,tags\n1,a\n",
            "0",
            "",
            "",
            false,
            true,
            true,
            "none",
            "comma",
            true,
            "csv",
        )
        .unwrap_err();
        assert_eq!(err, "column indices are 1-based (>= 1)");
    }

    #[test]
    fn bad_enum_values_are_rejected() {
        let err = dedupe_within_cells(
            "tags\na\n",
            "",
            "",
            "",
            false,
            true,
            true,
            "sideways",
            "comma",
            true,
            "csv",
        )
        .unwrap_err();
        assert_eq!(err, "sort_items must be none/asc/desc, got 'sideways'");
        let err = dedupe_within_cells(
            "tags\na\n",
            "",
            "",
            "",
            false,
            true,
            true,
            "none",
            "comma",
            true,
            "report",
        )
        .unwrap_err();
        assert_eq!(err, "output must be csv/stats, got 'report'");
        let err = dedupe_within_cells(
            "tags\na\n",
            "",
            "",
            "",
            false,
            true,
            true,
            "none",
            "colon",
            true,
            "csv",
        )
        .unwrap_err();
        assert_eq!(
            err,
            "delimiter must be comma/tab/semicolon/pipe, got 'colon'"
        );
    }

    #[test]
    fn ragged_rows_are_preserved_row_by_row() {
        let out = run("a,b,c\n\"x, x\"\n\"y, y\",\"z, z\",\"w, w\"\n");
        assert_eq!(out, "a,b,c\nx\ny,z,w\n");
    }
}
