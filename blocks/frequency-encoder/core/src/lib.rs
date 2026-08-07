//! frequency-encoder core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Frequency (count) encoding: replace a categorical column with how often each value occurs,
//! turning a high-cardinality text column into a single numeric feature without the column
//! explosion of one-hot encoding. The count can be emitted raw, as a share of the rows
//! (0–1), as a percentage, or log-scaled to compress very skewed distributions.

use std::collections::HashMap;

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
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
                    "delimiter must be a single char or tab/comma/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// Resolve a column spec (a header name, or a 1-based index) to a 0-based column index.
/// If a header row is present its names win over index parsing (so a numeric column name
/// like "2020" is matched as a name, not treated as position 2020).
fn resolve_col(spec: &str, header: Option<&csv::StringRecord>, width: usize) -> Result<usize, String> {
    let s = spec.trim();
    if s.is_empty() {
        return Err("column selector is empty".into());
    }
    if let Some(h) = header {
        if let Some(i) = h.iter().position(|c| c.trim() == s) {
            return Ok(i);
        }
    }
    match s.parse::<usize>() {
        Ok(n) if n >= 1 && n <= width => Ok(n - 1),
        Ok(_) => Err(format!(
            "column '{s}' is out of range (the CSV has {width} column(s))"
        )),
        Err(_) => Err(format!(
            "column '{s}' not found — pass a header name or a 1-based column number"
        )),
    }
}

/// Column-name suffix used by `output = "append"`, per encoding mode.
fn mode_suffix(mode: &str) -> Result<&'static str, String> {
    Ok(match mode {
        "count" | "" => "_count",
        "frequency" => "_freq",
        "percent" => "_pct",
        "log-count" => "_logcount",
        other => {
            return Err(format!(
                "mode must be 'count', 'frequency', 'percent', or 'log-count', got '{other}'"
            ))
        }
    })
}

fn fmt_num(v: f64, decimals: usize) -> String {
    format!("{v:.decimals$}")
}

/// Render a raw occurrence count in the requested mode.
/// `total` is the number of counted rows (the denominator for frequency/percent).
fn render(count: f64, total: f64, mode: &str, decimals: usize) -> String {
    match mode {
        "frequency" => fmt_num(if total > 0.0 { count / total } else { 0.0 }, decimals),
        "percent" => fmt_num(
            if total > 0.0 { 100.0 * count / total } else { 0.0 },
            decimals,
        ),
        "log-count" => fmt_num(count.ln_1p(), decimals),
        // "count" (and the empty default): a plain integer, unaffected by `decimals`.
        _ => format!("{count}"),
    }
}

/// Value used for rows whose category cell is blank, when those rows are not counted.
fn blank_value(blank: &str, mode: &str, decimals: usize) -> Result<String, String> {
    Ok(match blank {
        // "count" is handled by the caller (blanks become their own category).
        "count" | "" => String::new(),
        "nan" => "NaN".to_string(),
        "zero" => render(0.0, 1.0, mode, decimals),
        other => {
            return Err(format!(
                "blank must be 'count', 'nan', or 'zero', got '{other}'"
            ))
        }
    })
}

/// Frequency-encode one categorical column of a CSV.
///
/// - `column`: a header name, or a 1-based column index.
/// - `mode`: `"count"` (raw occurrences), `"frequency"` (share of rows, 0–1),
///   `"percent"` (share × 100), or `"log-count"` (`ln(1 + count)`).
/// - `output`: `"replace"` (overwrite the column in place) or `"append"` (keep it and add a
///   `<name>_count` / `_freq` / `_pct` / `_logcount` column).
/// - `blank`: how to treat blank cells — `"count"` (their own category, counted like any
///   other value), `"nan"`, or `"zero"`. With `"nan"`/`"zero"` blank rows are excluded from
///   the counts and from the frequency denominator.
/// - `min_count`: categories occurring fewer than this many times are pooled into a single
///   rare group and all share the pooled count. `0`/`1` disables pooling.
/// - `case_sensitive`: when false, values differing only in case are counted together.
/// - `decimals`: rounding for frequency / percent / log-count values (0..=15).
#[allow(clippy::too_many_arguments)]
pub fn encode(
    data: &str,
    column: &str,
    mode: &str,
    output: &str,
    blank: &str,
    min_count: usize,
    case_sensitive: bool,
    decimals: usize,
    has_header: bool,
    delimiter: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let suffix = mode_suffix(mode)?;
    let mode = if mode.is_empty() { "count" } else { mode };
    match output {
        "replace" | "append" | "" => {}
        other => return Err(format!("output must be 'replace' or 'append', got '{other}'")),
    }
    let decimals = decimals.min(15);
    // Validate the blank selector up front.
    blank_value(blank, mode, decimals)?;
    let count_blanks = matches!(blank, "count" | "");

    let delim = delim_byte(delimiter)?;
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

    let header = if has_header { records.first() } else { None };
    let col_idx = resolve_col(column, header, width)?;
    let col_name = match header {
        Some(h) => h.get(col_idx).unwrap_or("").trim().to_string(),
        None => format!("col{}", col_idx + 1),
    };
    let data_start = if has_header { 1 } else { 0 };
    if records.len() <= data_start {
        return Err("no data rows found — the CSV has only a header row".into());
    }

    // Grouping key for one cell: trimmed, and case-folded when case_sensitive is off.
    let key_of = |cell: &str| -> String {
        let t = cell.trim();
        if case_sensitive {
            t.to_string()
        } else {
            t.to_lowercase()
        }
    };

    // Pass 1: count each distinct value.
    let mut counts: HashMap<String, f64> = HashMap::new();
    let mut total = 0f64;
    for rec in &records[data_start..] {
        let cell = rec.get(col_idx).unwrap_or("");
        if cell.trim().is_empty() && !count_blanks {
            continue;
        }
        *counts.entry(key_of(cell)).or_insert(0.0) += 1.0;
        total += 1.0;
    }

    // Optional rare-category pooling: every category below `min_count` shares one pooled count,
    // so rare values collapse into a single "rare" level instead of many noisy singletons.
    if min_count >= 2 {
        let pooled: f64 = counts.values().filter(|c| (**c as usize) < min_count).sum();
        if pooled > 0.0 {
            for c in counts.values_mut() {
                if (*c as usize) < min_count {
                    *c = pooled;
                }
            }
        }
    }

    // Pass 2: emit rows.
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(vec![]);
    for (i, rec) in records.iter().enumerate() {
        let mut fields: Vec<String> = rec.iter().map(|s| s.to_string()).collect();
        while fields.len() <= col_idx {
            fields.push(String::new());
        }

        if has_header && i == 0 {
            if output == "append" {
                fields.push(format!("{col_name}{suffix}"));
            }
            wtr.write_record(&fields)
                .map_err(|e| format!("CSV write error: {e}"))?;
            continue;
        }

        let cell = rec.get(col_idx).unwrap_or("");
        let encoded = if cell.trim().is_empty() && !count_blanks {
            blank_value(blank, mode, decimals)?
        } else {
            let n = *counts.get(&key_of(cell)).unwrap_or(&0.0);
            render(n, total, mode, decimals)
        };

        if output == "append" {
            fields.push(encoded);
        } else {
            fields[col_idx] = encoded;
        }
        wtr.write_record(&fields)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // product_id counts: X=3, Y=2, Z=1 over 6 data rows.
    const D: &str = "product_id,price\nX,10\nY,20\nX,30\nZ,40\nX,50\nY,60";

    #[test]
    fn count_replace() {
        let out = encode(D, "product_id", "count", "replace", "count", 0, true, 4, true, ",").unwrap();
        assert_eq!(
            out,
            "product_id,price\n3,10\n2,20\n3,30\n1,40\n3,50\n2,60\n"
        );
    }

    #[test]
    fn frequency_is_share_of_rows() {
        let out = encode(D, "product_id", "frequency", "replace", "count", 0, true, 4, true, ",").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[1], "0.5000,10"); // X: 3/6
        assert_eq!(lines[2], "0.3333,20"); // Y: 2/6
        assert_eq!(lines[4], "0.1667,40"); // Z: 1/6
    }

    #[test]
    fn percent_mode_and_append_column() {
        let out = encode(D, "product_id", "percent", "append", "count", 0, true, 2, true, ",").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "product_id,price,product_id_pct");
        assert_eq!(lines[1], "X,10,50.00");
        assert_eq!(lines[4], "Z,40,16.67");
    }

    #[test]
    fn log_count_compresses_skew() {
        let out = encode(D, "product_id", "log-count", "replace", "count", 0, true, 4, true, ",").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[1], "1.3863,10"); // ln(1+3)
        assert_eq!(lines[4], "0.6931,40"); // ln(1+1)
    }

    #[test]
    fn column_by_index_without_header() {
        let d = "X,10\nY,20\nX,30";
        let out = encode(d, "1", "count", "replace", "count", 0, true, 4, false, ",").unwrap();
        assert_eq!(out, "2,10\n1,20\n2,30\n");
    }

    #[test]
    fn case_insensitive_groups_values() {
        let d = "city\nParis\nPARIS\nparis\nRome";
        let sensitive = encode(d, "city", "count", "replace", "count", 0, true, 4, true, ",").unwrap();
        assert_eq!(sensitive, "city\n1\n1\n1\n1\n");
        let folded = encode(d, "city", "count", "replace", "count", 0, false, 4, true, ",").unwrap();
        assert_eq!(folded, "city\n3\n3\n3\n1\n");
    }

    #[test]
    fn blank_handling() {
        let d = "city,n\nA,1\n,2\nA,3";
        // counted as their own category: blank appears once, A twice, total 3
        let counted = encode(d, "city", "count", "replace", "count", 0, true, 4, true, ",").unwrap();
        assert_eq!(counted.lines().nth(2).unwrap(), "1,2");
        // nan: blank rows excluded from counts and from the denominator
        let nan = encode(d, "city", "frequency", "replace", "nan", 0, true, 2, true, ",").unwrap();
        assert_eq!(nan.lines().nth(1).unwrap(), "1.00,1"); // A: 2/2 of the counted rows
        assert_eq!(nan.lines().nth(2).unwrap(), "NaN,2");
        let zero = encode(d, "city", "count", "replace", "zero", 0, true, 4, true, ",").unwrap();
        assert_eq!(zero.lines().nth(2).unwrap(), "0,2");
    }

    #[test]
    fn min_count_pools_rare_categories() {
        // X=3, Y=2, Z=1 — with min_count=3, Y and Z pool into one group of 3
        let out = encode(D, "product_id", "count", "replace", "count", 3, true, 4, true, ",").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[1], "3,10"); // X keeps its own count
        assert_eq!(lines[2], "3,20"); // Y pooled (2 + 1)
        assert_eq!(lines[4], "3,40"); // Z pooled
    }

    #[test]
    fn tab_delimiter_round_trips() {
        let d = "city\tn\nA\t1\nA\t2\nB\t3";
        let out = encode(d, "city", "count", "replace", "count", 0, true, 4, true, "tab").unwrap();
        assert_eq!(out, "city\tn\n2\t1\n2\t2\n1\t3\n");
    }

    #[test]
    fn errors() {
        // empty input
        assert!(encode("   ", "a", "count", "replace", "count", 0, true, 4, true, ",").is_err());
        // unknown column name
        assert!(encode(D, "nope", "count", "replace", "count", 0, true, 4, true, ",").is_err());
        // out-of-range index
        assert!(encode(D, "9", "count", "replace", "count", 0, true, 4, false, ",").is_err());
        // bad mode / output / blank / delimiter
        assert!(encode(D, "product_id", "median", "replace", "count", 0, true, 4, true, ",").is_err());
        assert!(encode(D, "product_id", "count", "sideways", "count", 0, true, 4, true, ",").is_err());
        assert!(encode(D, "product_id", "count", "replace", "maybe", 0, true, 4, true, ",").is_err());
        assert!(encode(D, "product_id", "count", "replace", "count", 0, true, 4, true, "::").is_err());
        // header-only input has no data rows
        assert!(encode("product_id,price", "product_id", "count", "replace", "count", 0, true, 4, true, ",").is_err());
    }
}
