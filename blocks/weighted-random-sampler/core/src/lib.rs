//! weighted-random-sampler core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! Draws a weighted random sample of rows from a CSV or JSON-array dataset. Each
//! row is selected with probability proportional to a numeric weight field, with
//! or without replacement. Randomness is a tiny in-house seeded PRNG
//! (splitmix64), so every surface — chat, CLI, page, tests — is deterministic for
//! a given `seed`; change `seed` for a different draw. No OS RNG / `getrandom`.
//!
//! Without-replacement draws use the Efraimidis–Spirakis A-Res scheme: each row
//! `i` with weight `w_i` gets key `u_i^(1/w_i)` for a uniform `u_i ∈ (0,1]`, and
//! the `n` largest keys form the sample. With replacement, each of the `n` draws
//! is an independent roulette-wheel pick over the cumulative weights.

use serde_json::Value;

/// splitmix64 — a tiny deterministic PRNG. `next_u64()` yields a u64 stream.
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        // Avoid a zero state producing a degenerate stream.
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform double in `[0, 1)` (53-bit mantissa).
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / 9_007_199_254_740_992.0 // 2^53
    }
    /// Uniform double in `(0, 1)` — never 0 or 1, safe as the base of `u^(1/w)`.
    fn unit_pos(&mut self) -> f64 {
        ((self.next_u64() >> 11) + 1) as f64 / 9_007_199_254_740_993.0 // 2^53 + 1
    }
}

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
                    "delimiter must be a single char or comma/tab/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// Resolve a weight field to a 0-based column index for CSV. Accepts a 1-based
/// numeric index or a header name (when a header is present).
fn resolve_column(name: &str, header: Option<&csv::StringRecord>) -> Result<usize, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("weight_field is required — the column holding each row's weight".into());
    }
    if let Ok(n) = name.parse::<usize>() {
        if n == 0 {
            return Err("weight_field index is 1-based (>= 1)".into());
        }
        return Ok(n - 1);
    }
    match header {
        Some(h) => h
            .iter()
            .position(|c| c == name)
            .ok_or_else(|| format!("weight_field '{name}' not found in the header")),
        None => Err(format!(
            "weight_field '{name}' is not a number and there is no header to match names"
        )),
    }
}

/// Parse one cell/value into a non-negative finite weight.
fn parse_weight(raw: &str, row_1based: usize) -> Result<f64, String> {
    let t = raw.trim();
    let w: f64 = t
        .parse()
        .map_err(|_| format!("weight in row {row_1based} is not a number: '{raw}'"))?;
    if !w.is_finite() || w < 0.0 {
        return Err(format!(
            "weight in row {row_1based} must be a non-negative finite number, got {w}"
        ));
    }
    Ok(w)
}

/// Core selection: return the indices (into `weights`) chosen. For without
/// replacement the result is sorted ascending (original order); for with
/// replacement it is in draw order (a row may repeat).
fn select_indices(
    weights: &[f64],
    n: usize,
    replacement: bool,
    rng: &mut Rng,
) -> Result<Vec<usize>, String> {
    let total: f64 = weights.iter().sum();
    if total <= 0.0 {
        return Err("all weights are zero — nothing can be sampled".into());
    }
    if n == 0 {
        return Ok(Vec::new());
    }
    if replacement {
        // Cumulative weights + roulette-wheel draws.
        let mut cum = Vec::with_capacity(weights.len());
        let mut acc = 0.0;
        for w in weights {
            acc += *w;
            cum.push(acc);
        }
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            let r = rng.unit() * total;
            // First index whose cumulative weight is > r.
            let idx = cum.partition_point(|&c| c <= r).min(weights.len() - 1);
            out.push(idx);
        }
        Ok(out)
    } else {
        let positive = weights.iter().filter(|w| **w > 0.0).count();
        if n > positive {
            return Err(format!(
                "cannot draw {n} rows without replacement — only {positive} row(s) have a positive weight"
            ));
        }
        // Efraimidis–Spirakis: key_i = u_i^(1/w_i); keep the n largest keys.
        let mut keyed: Vec<(f64, usize)> = weights
            .iter()
            .enumerate()
            .filter(|(_, w)| **w > 0.0)
            .map(|(i, w)| (rng.unit_pos().powf(1.0 / *w), i))
            .collect();
        keyed.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        keyed.truncate(n);
        let mut idx: Vec<usize> = keyed.into_iter().map(|(_, i)| i).collect();
        idx.sort_unstable();
        Ok(idx)
    }
}

fn sample_csv(
    data: &str,
    weight_field: &str,
    n: usize,
    replacement: bool,
    seed: u64,
    header: bool,
    delimiter: &str,
) -> Result<String, String> {
    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delim)
        .from_reader(data.as_bytes());

    let mut records: Vec<csv::StringRecord> = Vec::new();
    for r in rdr.records() {
        records.push(r.map_err(|e| format!("CSV parse error: {e}"))?);
    }
    if records.is_empty() {
        return Err("no rows found in the CSV input".into());
    }

    let (header_rec, data_rows): (Option<csv::StringRecord>, &[csv::StringRecord]) = if header {
        (Some(records[0].clone()), &records[1..])
    } else {
        (None, &records[..])
    };
    if data_rows.is_empty() {
        return Err("no data rows to sample (only a header was present)".into());
    }

    let col = resolve_column(weight_field, header_rec.as_ref())?;
    let mut weights = Vec::with_capacity(data_rows.len());
    for (i, rec) in data_rows.iter().enumerate() {
        let cell = rec
            .get(col)
            .ok_or_else(|| format!("row {} has no column {} (weight_field)", i + 1, col + 1))?;
        weights.push(parse_weight(cell, i + 1)?);
    }

    let mut rng = Rng::new(seed);
    let picks = select_indices(&weights, n, replacement, &mut rng)?;

    // Serialize back out with the same delimiter, header preserved.
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .from_writer(Vec::new());
    if let Some(h) = &header_rec {
        wtr.write_record(h).map_err(|e| e.to_string())?;
    }
    for &i in &picks {
        wtr.write_record(&data_rows[i]).map_err(|e| e.to_string())?;
    }
    let bytes = wtr.into_inner().map_err(|e| e.to_string())?;
    let mut s = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    // csv writer terminates every record with a newline; trim the trailing one.
    if s.ends_with('\n') {
        s.pop();
        if s.ends_with('\r') {
            s.pop();
        }
    }
    Ok(s)
}

fn sample_json(
    data: &str,
    weight_field: &str,
    n: usize,
    replacement: bool,
    seed: u64,
) -> Result<String, String> {
    let key = weight_field.trim();
    if key.is_empty() {
        return Err("weight_field is required — the object key holding each row's weight".into());
    }
    let parsed: Value = serde_json::from_str(data).map_err(|e| format!("invalid JSON: {e}"))?;
    let arr = parsed
        .as_array()
        .ok_or("JSON input must be an array of objects")?;
    if arr.is_empty() {
        return Err("no rows found in the JSON array".into());
    }

    let mut weights = Vec::with_capacity(arr.len());
    for (i, item) in arr.iter().enumerate() {
        let obj = item
            .as_object()
            .ok_or_else(|| format!("array element {} is not a JSON object", i + 1))?;
        let v = obj
            .get(key)
            .ok_or_else(|| format!("row {} has no '{key}' field (weight_field)", i + 1))?;
        let raw = match v {
            Value::Number(nn) => nn.to_string(),
            Value::String(s) => s.clone(),
            _ => {
                return Err(format!(
                    "row {} '{key}' must be a number or numeric string",
                    i + 1
                ))
            }
        };
        weights.push(parse_weight(&raw, i + 1)?);
    }

    let mut rng = Rng::new(seed);
    let picks = select_indices(&weights, n, replacement, &mut rng)?;
    let out: Vec<&Value> = picks.iter().map(|&i| &arr[i]).collect();
    serde_json::to_string_pretty(&out).map_err(|e| e.to_string())
}

/// Draw a weighted random sample of `n` rows from `data`.
///
/// * `format` — `"csv"` or `"json"` (a JSON array of objects).
/// * `weight_field` — CSV: header name or 1-based column index. JSON: object key.
/// * `n` — number of rows to draw. Without replacement it is capped at the count
///   of positive-weight rows (error if it exceeds it); with replacement it may
///   exceed the row count.
/// * `replacement` — allow the same row to be drawn more than once.
/// * `seed` — PRNG seed for a reproducible draw.
/// * `header` / `delimiter` — CSV only.
#[allow(clippy::too_many_arguments)]
pub fn sample(
    data: &str,
    format: &str,
    weight_field: &str,
    n: usize,
    replacement: bool,
    seed: u64,
    header: bool,
    delimiter: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input data is empty".into());
    }
    match format {
        "csv" | "" => sample_csv(data, weight_field, n, replacement, seed, header, delimiter),
        "json" => sample_json(data, weight_field, n, replacement, seed),
        other => Err(format!("format must be 'csv' or 'json', got '{other}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: &str = "name,weight\nAlice,1\nBob,1\nCarol,1\nDan,1";

    #[test]
    fn csv_without_replacement_is_deterministic_and_preserves_order() {
        let out = sample(CSV, "csv", "weight", 2, false, 42, true, "comma").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "name,weight");
        assert_eq!(lines.len(), 3); // header + 2
        let again = sample(CSV, "csv", "weight", 2, false, 42, true, "comma").unwrap();
        assert_eq!(out, again, "same seed must reproduce the draw");
    }

    #[test]
    fn heavy_weight_dominates_with_replacement() {
        let csv = "name,weight\nHeavy,1000\nLight,1";
        let out = sample(csv, "csv", "weight", 10, true, 7, true, "comma").unwrap();
        let heavy = out.matches("Heavy").count();
        assert!(
            heavy >= 9,
            "expected Heavy to dominate, got {heavy}/10:\n{out}"
        );
    }

    #[test]
    fn with_replacement_can_exceed_row_count() {
        let out = sample(CSV, "csv", "weight", 6, true, 3, true, "comma").unwrap();
        assert_eq!(out.lines().count(), 7); // header + 6
    }

    #[test]
    fn json_by_key_deterministic() {
        let json = r#"[{"id":"a","w":5},{"id":"b","w":5},{"id":"c","w":5}]"#;
        let out = sample(json, "json", "w", 2, false, 1, true, "comma").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 2);
        let again = sample(json, "json", "w", 2, false, 1, true, "comma").unwrap();
        assert_eq!(out, again);
    }

    #[test]
    fn json_numeric_string_weight_ok() {
        let json = r#"[{"id":"a","w":"2.5"},{"id":"b","w":"0.5"}]"#;
        let out = sample(json, "json", "w", 1, false, 9, true, "comma").unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&out)
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn without_replacement_over_positive_count_errors() {
        let csv = "name,weight\nA,1\nB,0\nC,0";
        let err = sample(csv, "csv", "weight", 2, false, 1, true, "comma").unwrap_err();
        assert!(err.contains("only 1"), "got: {err}");
    }

    #[test]
    fn all_zero_weights_error() {
        let csv = "name,weight\nA,0\nB,0";
        let err = sample(csv, "csv", "weight", 1, false, 1, true, "comma").unwrap_err();
        assert!(err.contains("all weights are zero"), "got: {err}");
    }

    #[test]
    fn non_numeric_weight_errors() {
        let csv = "name,weight\nA,heavy";
        let err = sample(csv, "csv", "weight", 1, true, 1, true, "comma").unwrap_err();
        assert!(err.contains("not a number"), "got: {err}");
    }

    #[test]
    fn unknown_weight_column_errors() {
        let err = sample(CSV, "csv", "nope", 1, true, 1, true, "comma").unwrap_err();
        assert!(err.contains("not found in the header"), "got: {err}");
    }

    #[test]
    fn bad_format_errors() {
        let err = sample(CSV, "xml", "weight", 1, true, 1, true, "comma").unwrap_err();
        assert!(err.contains("format must be"), "got: {err}");
    }

    #[test]
    fn json_must_be_array() {
        let err = sample(r#"{"w":1}"#, "json", "w", 1, true, 1, true, "comma").unwrap_err();
        assert!(err.contains("must be an array"), "got: {err}");
    }
}
