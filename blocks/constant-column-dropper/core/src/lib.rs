//! gizza-ai/constant-column-dropper core — pure compute, shared by the chat
//! skill block and the web page. No wafer/wasm-bindgen deps.
//!
//! Detects and removes **zero-variance columns** in a CSV/table: columns that
//! carry a single repeated value down every data row (or that are entirely
//! empty). Constancy is defined as *one distinct value*, not `variance == 0` —
//! the distinct-value rule generalizes to text columns, which the statistical
//! formula cannot handle.
//!
//! The strict case is the default. `dominance` widens it to the *near*-constant
//! case: a column is dropped when its most frequent value covers at least that
//! percentage of the considered rows (100 = strictly constant, 95 = "95% of the
//! rows say the same thing"). Empty cells either count as their own value or are
//! skipped, mirroring the two defensible conventions in table tooling.
//!
//! Output is a human `report`, the cleaned `csv`, or `json` per-column metrics.
//! This is the zero-variance counterpart to `duplicate-column-detector` (which
//! compares columns against each other rather than against themselves).

use std::collections::HashMap;

/// Parse a delimiter spec: a single char, or a friendly name.
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
                    "delimiter must be a single char or comma/tab/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// Normalize a cell for *comparison* only (never for display).
fn normalize(v: &str, ignore_case: bool, ignore_ws: bool) -> String {
    let mut s = v.to_string();
    if ignore_ws {
        s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    if ignore_case {
        s = s.to_lowercase();
    }
    s
}

/// Column label for reporting: `"name" (col N)` when a non-blank header exists,
/// else `col N` (1-based).
fn col_label(idx: usize, header: Option<&csv::StringRecord>) -> String {
    match header.and_then(|h| h.get(idx)) {
        Some(name) if !name.trim().is_empty() => format!("\"{}\" (col {})", name, idx + 1),
        _ => format!("col {}", idx + 1),
    }
}

/// Bare column name for structured output: the header value, or `column N`.
fn col_name(idx: usize, header: Option<&csv::StringRecord>) -> String {
    match header.and_then(|h| h.get(idx)) {
        Some(name) if !name.trim().is_empty() => name.to_string(),
        _ => format!("column {}", idx + 1),
    }
}

/// Why a column was (or was not) dropped.
#[derive(PartialEq)]
enum Verdict {
    /// Every considered cell holds the same value.
    Constant,
    /// The top value covers >= `dominance`% of considered cells, but not all.
    NearConstant,
    /// No considered cells at all (the column is entirely empty).
    AllEmpty,
    /// Would have been dropped, but the column is named in `keep`.
    Protected,
    /// Varies enough to survive.
    Varies,
}

/// Per-column measurement.
struct ColStat {
    idx: usize,
    /// Number of distinct (normalized) values among the considered cells.
    distinct: usize,
    /// Number of cells that took part in the count.
    considered: usize,
    /// The most frequent value as it appeared in the data (first occurrence
    /// wins ties, so the result is deterministic), and how often it occurred.
    top_value: String,
    top_count: usize,
    verdict: Verdict,
}

impl ColStat {
    /// Share of considered cells taken by the top value, as a percentage.
    fn top_share(&self) -> f64 {
        if self.considered == 0 {
            0.0
        } else {
            self.top_count as f64 * 100.0 / self.considered as f64
        }
    }
    fn dropped(&self) -> bool {
        matches!(
            self.verdict,
            Verdict::Constant | Verdict::NearConstant | Verdict::AllEmpty
        )
    }
    fn reason(&self) -> &'static str {
        match self.verdict {
            Verdict::Constant => "constant",
            Verdict::NearConstant => "near-constant",
            Verdict::AllEmpty => "all cells are empty",
            Verdict::Protected => "protected by keep",
            Verdict::Varies => "varies",
        }
    }
}

/// Does `keep` protect column `idx`? Entries match either a 1-based column
/// number or a header name (trimmed, case-insensitive).
fn is_protected(keep: &[String], idx: usize, header: Option<&csv::StringRecord>) -> bool {
    let name = header
        .and_then(|h| h.get(idx))
        .map(|n| n.trim().to_lowercase());
    keep.iter().any(|k| {
        if let Ok(n) = k.parse::<usize>() {
            if n == idx + 1 {
                return true;
            }
        }
        name.as_deref() == Some(k.as_str())
    })
}

/// Drop zero-variance (constant) columns from a CSV/table.
///
/// * `data` — the CSV/table text.
/// * `has_header` — treat the first row as column names (kept in CSV output).
/// * `delimiter` — input field separator (char, or comma/tab/semicolon/pipe).
/// * `dominance` — drop a column once its most frequent value covers at least
///   this percent of considered cells; 100 (default) means strictly constant.
/// * `empty_cells` — `"value"` counts an empty cell as its own value,
///   `"ignore"` skips empty cells before counting.
/// * `ignore_case` / `ignore_ws` — normalization applied before comparing.
/// * `keep` — comma-separated column names or 1-based indices never dropped.
/// * `output` — `report` (human summary), `csv` (cleaned table), or `json`.
#[allow(clippy::too_many_arguments)]
pub fn drop_constant(
    data: &str,
    has_header: bool,
    delimiter: &str,
    dominance: f64,
    empty_cells: &str,
    ignore_case: bool,
    ignore_ws: bool,
    keep: &str,
    output: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty — paste CSV/table text with one row per line".into());
    }
    let out_mode = match output.trim() {
        "" | "report" => "report",
        "csv" => "csv",
        "json" => "json",
        other => return Err(format!("output must be report, csv or json, got '{other}'")),
    };
    let skip_empty = match empty_cells.trim() {
        "" | "value" => false,
        "ignore" => true,
        other => {
            return Err(format!(
                "empty_cells must be value or ignore, got '{other}'"
            ))
        }
    };
    if !(50.0..=100.0).contains(&dominance) {
        return Err(format!(
            "dominance must be between 50 and 100 percent, got {dominance}"
        ));
    }
    let delim = delim_byte(delimiter)?;
    let keep_list: Vec<String> = keep
        .split(',')
        .map(|k| k.trim().to_lowercase())
        .filter(|k| !k.is_empty())
        .collect();

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> = rdr
        .records()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() {
        return Err("no rows found".into());
    }
    let width = records.iter().map(|r| r.len()).max().unwrap_or(0);
    if width == 0 {
        return Err("no columns found".into());
    }

    let header_owned = if has_header { records.first().cloned() } else { None };
    let header_ref = header_owned.as_ref();

    let data_start = if has_header { 1 } else { 0 };
    let data_records = &records[data_start.min(records.len())..];
    if data_records.is_empty() {
        return Err("no data rows found (only a header row) — add at least one data row".into());
    }

    // --- Measure every column ------------------------------------------------
    let mut stats: Vec<ColStat> = Vec::with_capacity(width);
    for c in 0..width {
        // Counts keyed by the normalized value; the stored string is the FIRST
        // raw spelling seen, so reports show the data as the user wrote it.
        let mut counts: HashMap<String, (String, usize)> = HashMap::new();
        let mut order: Vec<String> = Vec::new();
        let mut considered = 0usize;
        let mut nonblank = 0usize;
        for rec in data_records {
            let raw = rec.get(c).unwrap_or("");
            if raw.trim().is_empty() {
                if skip_empty {
                    continue;
                }
            } else {
                nonblank += 1;
            }
            considered += 1;
            let key = normalize(raw, ignore_case, ignore_ws);
            match counts.get_mut(&key) {
                Some(e) => e.1 += 1,
                None => {
                    counts.insert(key.clone(), (raw.to_string(), 1));
                    order.push(key);
                }
            }
        }
        // First-seen order breaks frequency ties deterministically.
        let (top_value, top_count) = order
            .iter()
            .map(|k| counts[k].clone())
            .max_by_key(|&(_, n)| n)
            .unwrap_or_default();
        let distinct = order.len();
        // An entirely blank column is degenerate in either empty-cell mode.
        let verdict = if nonblank == 0 {
            Verdict::AllEmpty
        } else if distinct == 1 {
            Verdict::Constant
        } else if top_count as f64 * 100.0 >= dominance * considered as f64 {
            Verdict::NearConstant
        } else {
            Verdict::Varies
        };
        let verdict = if verdict != Verdict::Varies && is_protected(&keep_list, c, header_ref) {
            Verdict::Protected
        } else {
            verdict
        };
        stats.push(ColStat {
            idx: c,
            distinct,
            considered,
            top_value,
            top_count,
            verdict,
        });
    }

    let dropped: Vec<&ColStat> = stats.iter().filter(|s| s.dropped()).collect();
    let kept: Vec<&ColStat> = stats.iter().filter(|s| !s.dropped()).collect();
    let protected: Vec<&ColStat> = stats
        .iter()
        .filter(|s| s.verdict == Verdict::Protected)
        .collect();

    match out_mode {
        "json" => {
            use serde_json::json;
            let cols: Vec<_> = stats
                .iter()
                .map(|s| {
                    json!({
                        "column": col_name(s.idx, header_ref),
                        "index": s.idx + 1,
                        "distinct_values": s.distinct,
                        "considered_rows": s.considered,
                        "top_value": s.top_value,
                        "top_count": s.top_count,
                        "top_share_percent": (s.top_share() * 100.0).round() / 100.0,
                        "dropped": s.dropped(),
                        "reason": s.reason(),
                    })
                })
                .collect();
            let v = json!({
                "columns": width,
                "data_rows": data_records.len(),
                "dominance_percent": dominance,
                "empty_cells": if skip_empty { "ignore" } else { "value" },
                "dropped_columns": dropped.len(),
                "kept_columns": kept.len(),
                "dropped": dropped
                    .iter()
                    .map(|s| col_name(s.idx, header_ref))
                    .collect::<Vec<_>>(),
                "kept": kept
                    .iter()
                    .map(|s| col_name(s.idx, header_ref))
                    .collect::<Vec<_>>(),
                "column_stats": cols,
            });
            serde_json::to_string_pretty(&v).map_err(|e| format!("json error: {e}"))
        }
        "csv" => {
            if kept.is_empty() {
                return Err(format!(
                    "every column is constant ({} of {} columns would be dropped) — nothing would remain; use output=report to inspect them, or list a column in keep",
                    dropped.len(),
                    width
                ));
            }
            let keep_idx: Vec<usize> = kept.iter().map(|s| s.idx).collect();
            let mut wtr = csv::WriterBuilder::new()
                .delimiter(delim)
                .flexible(true)
                .from_writer(vec![]);
            for rec in &records {
                let row: Vec<&str> = keep_idx.iter().map(|&c| rec.get(c).unwrap_or("")).collect();
                wtr.write_record(&row)
                    .map_err(|e| format!("CSV write error: {e}"))?;
            }
            let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
            String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
        }
        _ => {
            let mut out = String::new();
            out.push_str(&format!(
                "Scanned {} column{} across {} data row{} (dominance {}%).\n",
                width,
                if width == 1 { "" } else { "s" },
                data_records.len(),
                if data_records.len() == 1 { "" } else { "s" },
                fmt_pct(dominance),
            ));
            if dropped.is_empty() {
                out.push_str("No constant columns found — every column varies.\n");
            } else {
                out.push_str(&format!(
                    "Found {} constant column{}; {} column{} remain.\n\nConstant columns (dropped):\n",
                    dropped.len(),
                    if dropped.len() == 1 { "" } else { "s" },
                    kept.len(),
                    if kept.len() == 1 { "" } else { "s" },
                ));
                for s in &dropped {
                    if s.verdict == Verdict::AllEmpty {
                        out.push_str(&format!(
                            "  {}  =  all cells are empty\n",
                            col_label(s.idx, header_ref)
                        ));
                    } else {
                        out.push_str(&format!(
                            "  {}  =  \"{}\" in {}/{} rows ({}%)\n",
                            col_label(s.idx, header_ref),
                            s.top_value,
                            s.top_count,
                            s.considered,
                            fmt_pct(s.top_share()),
                        ));
                    }
                }
            }
            if !protected.is_empty() {
                out.push_str("\nProtected by keep (constant but not dropped):\n");
                for s in &protected {
                    out.push_str(&format!("  {}\n", col_label(s.idx, header_ref)));
                }
            }
            if !dropped.is_empty() {
                out.push_str("\nUse output=csv to get the table with those columns removed.\n");
            }
            Ok(out)
        }
    }
}

/// Percentages print without a trailing `.0` so reports read `100%`, not `100.0%`.
fn fmt_pct(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if (r - r.round()).abs() < f64::EPSILON {
        format!("{}", r.round() as i64)
    } else {
        format!("{r}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // `country` is constant, `notes` is entirely empty, `id`/`score` vary.
    const SAMPLE: &str = "id,country,score,notes\n1,US,10,\n2,US,20,\n3,US,30,\n4,US,40,";

    fn report(data: &str) -> String {
        drop_constant(data, true, "comma", 100.0, "value", true, true, "", "report").unwrap()
    }

    #[test]
    fn reports_constant_and_empty_columns() {
        let out = report(SAMPLE);
        assert!(out.contains("Scanned 4 columns across 4 data rows (dominance 100%)."), "{out}");
        assert!(out.contains("Found 2 constant columns; 2 columns remain."), "{out}");
        assert!(out.contains("\"country\" (col 2)  =  \"US\" in 4/4 rows (100%)"), "{out}");
        assert!(out.contains("\"notes\" (col 4)  =  all cells are empty"), "{out}");
    }

    #[test]
    fn csv_output_removes_constant_columns_exactly() {
        let out = drop_constant(SAMPLE, true, "comma", 100.0, "value", true, true, "", "csv")
            .unwrap();
        assert_eq!(out, "id,score\n1,10\n2,20\n3,30\n4,40\n");
    }

    #[test]
    fn nothing_dropped_when_every_column_varies() {
        let out = report("a,b\n1,2\n3,4");
        assert!(out.contains("No constant columns found"), "{out}");
    }

    #[test]
    fn dominance_below_100_catches_near_constant_columns() {
        // `flag` is "Y" in 3 of 4 rows = 75%.
        let data = "id,flag\n1,Y\n2,Y\n3,Y\n4,N";
        let strict =
            drop_constant(data, true, "comma", 100.0, "value", true, true, "", "report").unwrap();
        assert!(strict.contains("No constant columns found"), "{strict}");
        let loose =
            drop_constant(data, true, "comma", 75.0, "value", true, true, "", "report").unwrap();
        assert!(loose.contains("\"flag\" (col 2)  =  \"Y\" in 3/4 rows (75%)"), "{loose}");
    }

    #[test]
    fn empty_cells_ignore_makes_a_sparse_column_constant() {
        let data = "id,tier\n1,gold\n2,\n3,gold";
        let as_value =
            drop_constant(data, true, "comma", 100.0, "value", true, true, "", "report").unwrap();
        assert!(as_value.contains("No constant columns found"), "{as_value}");
        let ignored =
            drop_constant(data, true, "comma", 100.0, "ignore", true, true, "", "report").unwrap();
        assert!(ignored.contains("\"tier\" (col 2)  =  \"gold\" in 2/2 rows (100%)"), "{ignored}");
    }

    #[test]
    fn keep_protects_a_constant_column_by_name_and_index() {
        let out = drop_constant(SAMPLE, true, "comma", 100.0, "value", true, true, "country", "csv")
            .unwrap();
        assert_eq!(out, "id,country,score\n1,US,10\n2,US,20\n3,US,30\n4,US,40\n");
        let by_index =
            drop_constant(SAMPLE, true, "comma", 100.0, "value", true, true, "2", "csv").unwrap();
        assert_eq!(by_index, out);
        let rep =
            drop_constant(SAMPLE, true, "comma", 100.0, "value", true, true, "country", "report")
                .unwrap();
        assert!(rep.contains("Protected by keep (constant but not dropped):"), "{rep}");
    }

    #[test]
    fn normalization_toggles_change_the_verdict() {
        let data = "id,state\n1,NY\n2,ny \n3, Ny";
        let normalized = report(data);
        assert!(normalized.contains("\"state\" (col 2)"), "{normalized}");
        let raw =
            drop_constant(data, true, "comma", 100.0, "value", false, false, "", "report").unwrap();
        assert!(raw.contains("No constant columns found"), "{raw}");
    }

    #[test]
    fn json_output_reports_per_column_metrics() {
        let out = drop_constant(SAMPLE, true, "comma", 100.0, "value", true, true, "", "json")
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["columns"], 4);
        assert_eq!(v["dropped_columns"], 2);
        assert_eq!(v["kept_columns"], 2);
        assert_eq!(v["dropped"][0], "country");
        assert_eq!(v["column_stats"][1]["distinct_values"], 1);
        assert_eq!(v["column_stats"][1]["top_share_percent"], 100.0);
        assert_eq!(v["column_stats"][3]["reason"], "all cells are empty");
    }

    #[test]
    fn tab_delimiter_and_headerless_input() {
        let out = drop_constant("1\tUS\n2\tUS", false, "tab", 100.0, "value", true, true, "", "csv")
            .unwrap();
        assert_eq!(out, "1\n2\n");
    }

    // --- error paths ---------------------------------------------------------

    #[test]
    fn empty_input_is_an_error() {
        let err = drop_constant("", true, "comma", 100.0, "value", true, true, "", "report")
            .unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn csv_output_errors_when_every_column_is_constant() {
        let err = drop_constant(
            "a,b\nx,1\nx,1", true, "comma", 100.0, "value", true, true, "", "csv",
        )
        .unwrap_err();
        assert!(err.contains("every column is constant"), "{err}");
        assert!(err.contains("2 of 2 columns"), "{err}");
    }

    #[test]
    fn bad_dominance_and_bad_enums_are_errors() {
        let err = drop_constant("a\n1", true, "comma", 10.0, "value", true, true, "", "report")
            .unwrap_err();
        assert!(err.contains("dominance must be between 50 and 100"), "{err}");
        let err = drop_constant("a\n1", true, "comma", 100.0, "maybe", true, true, "", "report")
            .unwrap_err();
        assert!(err.contains("empty_cells must be value or ignore"), "{err}");
        let err = drop_constant("a\n1", true, "comma", 100.0, "value", true, true, "", "xml")
            .unwrap_err();
        assert!(err.contains("output must be report, csv or json"), "{err}");
    }

    #[test]
    fn header_only_input_is_an_error() {
        let err = drop_constant("a,b", true, "comma", 100.0, "value", true, true, "", "report")
            .unwrap_err();
        assert!(err.contains("no data rows found"), "{err}");
    }
}
