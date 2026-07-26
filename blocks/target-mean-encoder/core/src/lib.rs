//! target-mean-encoder core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Target (mean / impact) encoding: replace a categorical column with the mean of a numeric
//! target per category, turning the category into a single model-ready numeric feature.
//! Optional m-estimate smoothing shrinks rare-category means toward the overall (prior) mean;
//! optional leave-one-out excludes each row's own target to reduce target leakage.

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

fn fallback(unknown: &str, prior: f64, decimals: usize) -> Result<String, String> {
    Ok(match unknown {
        "global-mean" | "" => fmt_num(prior, decimals),
        "nan" => "NaN".to_string(),
        "zero" => fmt_num(0.0, decimals),
        other => {
            return Err(format!(
                "unknown must be 'global-mean', 'nan', or 'zero', got '{other}'"
            ))
        }
    })
}

fn fmt_num(v: f64, decimals: usize) -> String {
    format!("{v:.decimals$}")
}

/// Target-encode one categorical column against one numeric target column.
///
/// - `category` / `target`: a header name or a 1-based column index.
/// - `smoothing` (m ≥ 0): weight of the global prior in the blend
///   `enc = (category_sum + m·prior) / (category_count + m)`. `0` = raw category mean.
/// - `leave_one_out`: exclude each row's own target from its category stats (leakage control).
/// - `output`: `"replace"` (overwrite the category column in place) or `"append"`
///   (keep it and add a `<name>_target_enc` column).
/// - `unknown`: value for rows whose category cell is blank — `"global-mean"`, `"nan"`, `"zero"`.
/// - `decimals`: rounding for the encoded values (0..=15).
#[allow(clippy::too_many_arguments)]
pub fn encode(
    data: &str,
    category: &str,
    target: &str,
    smoothing: f64,
    leave_one_out: bool,
    output: &str,
    unknown: &str,
    decimals: usize,
    has_header: bool,
    delimiter: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    if smoothing < 0.0 || !smoothing.is_finite() {
        return Err(format!("smoothing must be a finite number ≥ 0, got {smoothing}"));
    }
    let decimals = decimals.min(15);
    match output {
        "replace" | "append" | "" => {}
        other => return Err(format!("output must be 'replace' or 'append', got '{other}'")),
    }
    // Validate the unknown selector up front.
    fallback(unknown, 0.0, decimals)?;

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
    let cat_idx = resolve_col(category, header, width)?;
    let tgt_idx = resolve_col(target, header, width)?;
    if cat_idx == tgt_idx {
        return Err("the category and target columns must be different".into());
    }

    let cat_name = match header {
        Some(h) => h.get(cat_idx).unwrap_or("").trim().to_string(),
        None => format!("col{}", cat_idx + 1),
    };
    let data_start = if has_header { 1 } else { 0 };

    // Pass 1: per-category count/sum of valid numeric targets + the global prior.
    use std::collections::HashMap;
    let mut cat_n: HashMap<String, f64> = HashMap::new();
    let mut cat_sum: HashMap<String, f64> = HashMap::new();
    let mut global_n = 0f64;
    let mut global_sum = 0f64;
    for rec in &records[data_start..] {
        let y = rec.get(tgt_idx).and_then(|s| s.trim().parse::<f64>().ok());
        // Every numeric target contributes to the global prior (the overall
        // target mean), including rows whose category cell is blank.
        if let Some(y) = y {
            global_n += 1.0;
            global_sum += y;
        }
        let cat = rec.get(cat_idx).unwrap_or("").trim();
        if cat.is_empty() {
            continue; // blank category → handled at output time via `unknown`
        }
        if let Some(y) = y {
            *cat_n.entry(cat.to_string()).or_insert(0.0) += 1.0;
            *cat_sum.entry(cat.to_string()).or_insert(0.0) += y;
        }
    }
    if global_n == 0.0 {
        return Err("no numeric target values found — is the target column correct?".into());
    }
    let prior = global_sum / global_n;

    // Pass 2: emit rows.
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(vec![]);
    for (i, rec) in records.iter().enumerate() {
        let mut fields: Vec<String> = rec.iter().map(|s| s.to_string()).collect();
        while fields.len() <= cat_idx.max(tgt_idx) {
            fields.push(String::new());
        }

        if has_header && i == 0 {
            if output == "append" {
                fields.push(format!("{cat_name}_target_enc"));
            }
            wtr.write_record(&fields).map_err(|e| format!("CSV write error: {e}"))?;
            continue;
        }

        let cat = rec.get(cat_idx).unwrap_or("").trim();
        let encoded = if cat.is_empty() {
            fallback(unknown, prior, decimals)?
        } else {
            let n = *cat_n.get(cat).unwrap_or(&0.0);
            let sum = *cat_sum.get(cat).unwrap_or(&0.0);
            let (n_eff, sum_eff) = if leave_one_out {
                match rec.get(tgt_idx).and_then(|s| s.trim().parse::<f64>().ok()) {
                    Some(y) => (n - 1.0, sum - y), // drop this row's own target
                    None => (n, sum),
                }
            } else {
                (n, sum)
            };
            if n_eff + smoothing <= 0.0 {
                // no other rows in the category and no smoothing weight → prior fallback
                fallback(unknown, prior, decimals)?
            } else {
                let enc = (sum_eff + smoothing * prior) / (n_eff + smoothing);
                fmt_num(enc, decimals)
            }
        };

        if output == "append" {
            fields.push(encoded);
        } else {
            fields[cat_idx] = encoded;
        }
        wtr.write_record(&fields).map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // City → churn(0/1). means: A=(1+0)/2=0.5, B=(1+1+0)/3=0.667, C=1/1=1.0
    const D: &str = "city,churn\nA,1\nB,1\nA,0\nB,1\nC,1\nB,0";

    #[test]
    fn raw_mean_replace() {
        let out = encode(D, "city", "churn", 0.0, false, "replace", "global-mean", 3, true, ",").unwrap();
        assert_eq!(
            out,
            "city,churn\n0.500,1\n0.667,1\n0.500,0\n0.667,1\n1.000,1\n0.667,0\n"
        );
    }

    #[test]
    fn append_keeps_original() {
        let out = encode(D, "city", "churn", 0.0, false, "append", "global-mean", 2, true, ",").unwrap();
        let first = out.lines().next().unwrap();
        assert_eq!(first, "city,churn,city_target_enc");
        assert!(out.lines().nth(1).unwrap().starts_with("A,1,0.50"));
    }

    #[test]
    fn smoothing_pulls_toward_prior() {
        // prior = 4/6 = 0.6667. City C (n=1) raw=1.0; with m=3 → (1 + 3*0.6667)/(1+3)=0.75
        let out = encode(D, "city", "churn", 3.0, false, "replace", "global-mean", 4, true, ",").unwrap();
        let c_row = out.lines().find(|l| l.starts_with("0.7500")).unwrap();
        assert!(c_row.starts_with("0.7500"), "got {c_row}");
    }

    #[test]
    fn column_by_index() {
        // no header, category = col 1, target = col 2
        let d = "A,1\nB,0\nA,1";
        let out = encode(d, "1", "2", 0.0, false, "replace", "global-mean", 1, false, ",").unwrap();
        assert_eq!(out, "1.0,1\n0.0,0\n1.0,1\n");
    }

    #[test]
    fn leave_one_out_excludes_own_row() {
        // A rows targets [1,0]; LOO for the first A = 0/1 = 0.0, second A = 1/1 = 1.0
        let out = encode(D, "city", "churn", 0.0, true, "replace", "global-mean", 3, true, ",").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[1], "0.000,1"); // first A: other A target is 0
        assert_eq!(lines[3], "1.000,0"); // second A: other A target is 1
    }

    #[test]
    fn blank_category_uses_unknown_fallback() {
        let d = "city,churn\nA,1\n,0\nA,1";
        // prior = (1+0+1)/3 = 0.6667
        let out = encode(d, "city", "churn", 0.0, false, "replace", "global-mean", 4, true, ",").unwrap();
        assert_eq!(out.lines().nth(2).unwrap(), "0.6667,0");
        let nan = encode(d, "city", "churn", 0.0, false, "replace", "nan", 4, true, ",").unwrap();
        assert_eq!(nan.lines().nth(2).unwrap(), "NaN,0");
    }

    #[test]
    fn errors() {
        assert!(encode("   ", "a", "b", 0.0, false, "replace", "global-mean", 6, true, ",").is_err());
        // target column has no numbers
        assert!(encode("a,b\nx,y", "a", "b", 0.0, false, "replace", "global-mean", 6, true, ",").is_err());
        // unknown column name
        assert!(encode("a,b\n1,2", "nope", "b", 0.0, false, "replace", "global-mean", 6, true, ",").is_err());
        // same column
        assert!(encode("a,b\nx,1", "a", "a", 0.0, false, "replace", "global-mean", 6, true, ",").is_err());
        // negative smoothing
        assert!(encode("a,b\nx,1", "a", "b", -1.0, false, "replace", "global-mean", 6, true, ",").is_err());
    }
}
