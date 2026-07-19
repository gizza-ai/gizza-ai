//! gizza-ai/downsample-timeseries core — pure compute, shared by the chat
//! skill block and the web page. No wafer/wasm-bindgen deps.
//!
//! Reduces a time-series to a target point count while preserving its visual
//! shape. Every algorithm SELECTS existing points (no interpolation), so the
//! output is the original rows/elements verbatim: CSV rows keep all columns
//! and exact formatting (header preserved), JSON keeps element values.
//!
//! Algorithms: `lttb` (Largest-Triangle-Three-Buckets, exact port of the
//! canonical flot-downsample bucket math), `minmax` (min + max per bucket),
//! `m4` (first/min/max/last per bucket), `nth` (uniform stride incl. both
//! endpoints).

const MAX_BYTES: usize = 2_000_000;
const MAX_POINTS: usize = 100_000;

/// Which original rows/elements to emit, plus per-row labels for errors.
enum Rows {
    Csv {
        header: Option<String>,
        lines: Vec<String>,
    },
    Json {
        elems: Vec<serde_json::Value>,
    },
}

struct Series {
    x: Vec<f64>,
    y: Vec<f64>,
    /// 1-based source line number per data row (CSV) — JSON uses element index.
    line_nos: Option<Vec<usize>>,
    rows: Rows,
}

impl Series {
    fn row_label(&self, i: usize) -> String {
        match &self.line_nos {
            Some(nos) => format!("line {}", nos[i]),
            None => format!("element {i}"),
        }
    }
}

/// Downsample `data` (CSV text or a JSON array) to about `points` points.
#[allow(clippy::too_many_arguments)]
pub fn downsample(
    data: &str,
    algorithm: &str,
    points: usize,
    x_column: &str,
    y_column: &str,
    header: bool,
    output: &str,
) -> Result<String, String> {
    if data.len() > MAX_BYTES {
        return Err(format!(
            "input is {} bytes; the cap is {MAX_BYTES} bytes (~2 MB) — split the series or trim unused columns",
            data.len()
        ));
    }
    if !(2..=MAX_POINTS).contains(&points) {
        return Err(format!("points must be between 2 and {MAX_POINTS}, got {points}"));
    }
    let algorithm = {
        let a = algorithm.trim();
        if a.is_empty() { "lttb" } else { a }
    };
    let output = {
        let o = output.trim();
        if o.is_empty() { "points" } else { o }
    };
    if !matches!(output, "points" | "indices") {
        return Err(format!("output must be 'points' or 'indices', got '{output}'"));
    }
    let trimmed = data.trim();
    if trimmed.is_empty() {
        return Err("data is empty — paste a CSV or JSON time-series".into());
    }

    let series = if trimmed.starts_with('[') {
        parse_json(trimmed, x_column, y_column)?
    } else {
        parse_csv(trimmed, x_column, y_column, header)?
    };
    let n = series.y.len();
    if n == 0 {
        return Err("no data points found in the input".into());
    }
    // LTTB (and any time-series view) requires x sorted ascending; equal x is allowed.
    for i in 1..n {
        if series.x[i] < series.x[i - 1] {
            return Err(format!(
                "x values must be non-decreasing (sorted by time): {} has x = {} after x = {} — sort the series first, or set x_column to 'index'",
                series.row_label(i),
                series.x[i],
                series.x[i - 1]
            ));
        }
    }

    let sel = match algorithm {
        "lttb" => lttb_indices(&series.x, &series.y, points),
        "minmax" => minmax_indices(&series.y, points),
        "m4" => {
            if points < 4 {
                return Err("algorithm m4 needs points >= 4 (it keeps first/min/max/last per bucket)".into());
            }
            m4_indices(&series.y, points)
        }
        "nth" => nth_indices(n, points),
        other => {
            return Err(format!(
                "unknown algorithm '{other}' — use lttb, minmax, m4, or nth"
            ))
        }
    };

    match output {
        "indices" => Ok(format!(
            "[{}]",
            sel.iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(",")
        )),
        _ => match &series.rows {
            Rows::Csv { header, lines } => {
                let mut out = Vec::with_capacity(sel.len() + 1);
                if let Some(h) = header {
                    out.push(h.as_str());
                }
                for &i in &sel {
                    out.push(lines[i].as_str());
                }
                Ok(out.join("\n"))
            }
            Rows::Json { elems } => {
                let body = sel
                    .iter()
                    .map(|&i| {
                        format!(
                            "  {}",
                            serde_json::to_string(&elems[i]).expect("re-serialize parsed JSON")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",\n");
                Ok(format!("[\n{body}\n]"))
            }
        },
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn parse_num(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// x accepts a number OR an ISO-8601 / RFC 3339 date/time (converted to epoch
/// seconds; date-only means midnight UTC).
fn parse_x(s: &str) -> Option<f64> {
    if let Some(v) = parse_num(s) {
        return Some(v);
    }
    let t = s.trim();
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(t) {
        return Some(dt.timestamp_millis() as f64 / 1000.0);
    }
    for fmt in [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M",
    ] {
        if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(t, fmt) {
            return Some(ndt.and_utc().timestamp_millis() as f64 / 1000.0);
        }
    }
    if let Ok(nd) = chrono::NaiveDate::parse_from_str(t, "%Y-%m-%d") {
        let ndt = nd.and_hms_opt(0, 0, 0)?;
        return Some(ndt.and_utc().timestamp() as f64);
    }
    None
}

/// Pick the delimiter with the most occurrences outside quotes (comma wins ties).
fn detect_delim(line: &str) -> char {
    let (mut commas, mut tabs, mut semis) = (0usize, 0usize, 0usize);
    let mut in_q = false;
    for c in line.chars() {
        match c {
            '"' => in_q = !in_q,
            ',' if !in_q => commas += 1,
            '\t' if !in_q => tabs += 1,
            ';' if !in_q => semis += 1,
            _ => {}
        }
    }
    if tabs > commas && tabs >= semis {
        '\t'
    } else if semis > commas && semis > tabs {
        ';'
    } else {
        ','
    }
}

/// Quote-aware field split (RFC-4180-style `""` escapes). Quoted fields may
/// not contain line breaks (documented limitation).
fn split_fields(line: &str, delim: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if in_q {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_q = false;
                }
            } else {
                cur.push(c);
            }
        } else if c == '"' {
            in_q = true;
        } else if c == delim {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    out.push(cur);
    out
}

/// Resolve a column spec (header name or 1-based number) to a 0-based index.
fn resolve_col(
    spec: &str,
    header: Option<&[String]>,
    ncols: usize,
    what: &str,
) -> Result<usize, String> {
    let s = spec.trim();
    if let Ok(k) = s.parse::<usize>() {
        if (1..=ncols).contains(&k) {
            return Ok(k - 1);
        }
        return Err(format!(
            "{what} column {k} is out of range — the data has {ncols} column(s)"
        ));
    }
    if let Some(h) = header {
        if let Some(i) = h.iter().position(|f| f.trim().eq_ignore_ascii_case(s)) {
            return Ok(i);
        }
        return Err(format!(
            "{what} column '{s}' not found in the header ({})",
            h.iter().map(|f| f.trim()).collect::<Vec<_>>().join(", ")
        ));
    }
    Err(format!(
        "{what} column '{s}': the data has no header row — use a 1-based column number instead"
    ))
}

fn parse_csv(data: &str, x_column: &str, y_column: &str, header: bool) -> Result<Series, String> {
    // Keep original lines verbatim for output; skip blank lines.
    let lines: Vec<(usize, &str)> = data
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.strip_suffix('\r').unwrap_or(l)))
        .filter(|(_, l)| !l.trim().is_empty())
        .collect();
    if lines.is_empty() {
        return Err("no data points found in the input".into());
    }
    let delim = detect_delim(lines[0].1);
    let first_fields = split_fields(lines[0].1, delim);
    // A first row counts as a header iff header==true and at least one of its
    // fields is neither a number nor a date — a fully numeric first row is data.
    let has_header = header
        && first_fields
            .iter()
            .any(|f| !f.trim().is_empty() && parse_x(f).is_none());
    let ncols = first_fields.len();
    let header_fields: Option<Vec<String>> = has_header.then(|| first_fields.clone());
    let data_start = usize::from(has_header);

    // Resolve columns.
    let y_spec = y_column.trim();
    let y_idx = if y_spec.is_empty() {
        if ncols >= 2 { 1 } else { 0 }
    } else {
        resolve_col(y_spec, header_fields.as_deref(), ncols, "y")?
    };
    let x_spec = x_column.trim();
    let x_idx: Option<usize> = if x_spec.eq_ignore_ascii_case("index") {
        None
    } else if x_spec.is_empty() {
        if ncols >= 2 { Some(0) } else { None }
    } else {
        Some(resolve_col(x_spec, header_fields.as_deref(), ncols, "x")?)
    };
    if x_idx == Some(y_idx) {
        return Err(format!(
            "x_column and y_column both resolve to column {} — pick different columns",
            y_idx + 1
        ));
    }

    let rows = &lines[data_start..];
    let mut x = Vec::with_capacity(rows.len());
    let mut y = Vec::with_capacity(rows.len());
    let mut kept_lines = Vec::with_capacity(rows.len());
    let mut line_nos = Vec::with_capacity(rows.len());
    for (i, (lineno, line)) in rows.iter().enumerate() {
        let f = split_fields(line, delim);
        let y_tok = f.get(y_idx).map(|s| s.trim()).unwrap_or("");
        let y_v = parse_num(y_tok).ok_or_else(|| {
            format!(
                "line {lineno}: '{y_tok}' is not a finite number (y column {})",
                y_idx + 1
            )
        })?;
        let x_v = match x_idx {
            None => i as f64,
            Some(xi) => {
                let x_tok = f.get(xi).map(|s| s.trim()).unwrap_or("");
                parse_x(x_tok).ok_or_else(|| {
                    format!(
                        "line {lineno}: '{x_tok}' is not a number or ISO-8601 date/time (x column {})",
                        xi + 1
                    )
                })?
            }
        };
        x.push(x_v);
        y.push(y_v);
        kept_lines.push((*line).to_string());
        line_nos.push(*lineno);
    }
    Ok(Series {
        x,
        y,
        line_nos: Some(line_nos),
        rows: Rows::Csv {
            header: has_header.then(|| lines[0].1.to_string()),
            lines: kept_lines,
        },
    })
}

fn json_num(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64().filter(|f| f.is_finite()),
        serde_json::Value::String(s) => parse_num(s),
        _ => None,
    }
}

fn json_x(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64().filter(|f| f.is_finite()),
        serde_json::Value::String(s) => parse_x(s),
        _ => None,
    }
}

/// Case-insensitive object key lookup (exact match first).
fn get_key<'a>(
    obj: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<&'a serde_json::Value> {
    if let Some(v) = obj.get(key) {
        return Some(v);
    }
    obj.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v)
}

const X_KEYS: [&str; 7] = ["x", "t", "time", "timestamp", "ts", "date", "datetime"];
const Y_KEYS: [&str; 4] = ["y", "v", "value", "val"];

fn parse_json(data: &str, x_column: &str, y_column: &str) -> Result<Series, String> {
    let v: serde_json::Value =
        serde_json::from_str(data).map_err(|e| format!("invalid JSON: {e}"))?;
    let arr = match v {
        serde_json::Value::Array(a) => a,
        _ => return Err("JSON input must be an array".into()),
    };
    if arr.is_empty() {
        return Err("JSON array is empty".into());
    }
    let x_spec = x_column.trim();
    let y_spec = y_column.trim();
    let x_is_index = x_spec.eq_ignore_ascii_case("index");
    let mut x = Vec::with_capacity(arr.len());
    let mut y = Vec::with_capacity(arr.len());

    match &arr[0] {
        serde_json::Value::Number(_) | serde_json::Value::String(_) => {
            // Plain list of y values; x = element index.
            for (i, e) in arr.iter().enumerate() {
                let y_v = json_num(e)
                    .ok_or_else(|| format!("element {i}: {e} is not a finite number"))?;
                x.push(i as f64);
                y.push(y_v);
            }
        }
        serde_json::Value::Array(_) => {
            // [x, y] pairs; column specs are 1-based positions.
            let pos = |spec: &str, default: usize, what: &str| -> Result<usize, String> {
                if spec.is_empty() {
                    return Ok(default);
                }
                match spec.parse::<usize>() {
                    Ok(k) if k >= 1 => Ok(k - 1),
                    _ => Err(format!(
                        "{what} column '{spec}': JSON pairs have no named keys — use a 1-based position like 1 or 2"
                    )),
                }
            };
            let y_idx = pos(y_spec, 1, "y")?;
            let x_idx = if x_is_index { None } else { Some(pos(x_spec, 0, "x")?) };
            if x_idx == Some(y_idx) {
                return Err(format!(
                    "x_column and y_column both resolve to position {} — pick different positions",
                    y_idx + 1
                ));
            }
            for (i, e) in arr.iter().enumerate() {
                let inner = e.as_array().ok_or_else(|| {
                    format!("element {i}: expected an [x, y] array like the first element, got {e}")
                })?;
                let y_e = inner.get(y_idx).ok_or_else(|| {
                    format!("element {i}: has no position {} (length {})", y_idx + 1, inner.len())
                })?;
                let y_v = json_num(y_e)
                    .ok_or_else(|| format!("element {i}: {y_e} is not a finite number"))?;
                let x_v = match x_idx {
                    None => i as f64,
                    Some(xi) => {
                        let x_e = inner.get(xi).ok_or_else(|| {
                            format!("element {i}: has no position {} (length {})", xi + 1, inner.len())
                        })?;
                        json_x(x_e).ok_or_else(|| {
                            format!("element {i}: {x_e} is not a number or ISO-8601 date/time")
                        })?
                    }
                };
                x.push(x_v);
                y.push(y_v);
            }
        }
        serde_json::Value::Object(first) => {
            // Objects: named keys, auto-detected when not given.
            let y_key = if !y_spec.is_empty() {
                y_spec.to_string()
            } else {
                Y_KEYS
                    .iter()
                    .find(|k| get_key(first, k).is_some())
                    .map(|k| k.to_string())
                    .ok_or_else(|| {
                        format!(
                            "couldn't find a value key (tried {}) — set y_column to your key name",
                            Y_KEYS.join(", ")
                        )
                    })?
            };
            let x_key: Option<String> = if x_is_index {
                None
            } else if !x_spec.is_empty() {
                Some(x_spec.to_string())
            } else {
                X_KEYS
                    .iter()
                    .find(|k| get_key(first, k).is_some())
                    .map(|k| k.to_string())
            };
            if let Some(xk) = &x_key {
                if xk.eq_ignore_ascii_case(&y_key) {
                    return Err(format!(
                        "x_column and y_column both resolve to key '{y_key}' — pick different keys"
                    ));
                }
            }
            for (i, e) in arr.iter().enumerate() {
                let obj = e.as_object().ok_or_else(|| {
                    format!("element {i}: expected an object like the first element, got {e}")
                })?;
                let y_e = get_key(obj, &y_key)
                    .ok_or_else(|| format!("element {i}: has no key '{y_key}'"))?;
                let y_v = json_num(y_e).ok_or_else(|| {
                    format!("element {i}: '{y_key}' = {y_e} is not a finite number")
                })?;
                let x_v = match &x_key {
                    None => i as f64,
                    Some(xk) => {
                        let x_e = get_key(obj, xk)
                            .ok_or_else(|| format!("element {i}: has no key '{xk}'"))?;
                        json_x(x_e).ok_or_else(|| {
                            format!(
                                "element {i}: '{xk}' = {x_e} is not a number or ISO-8601 date/time"
                            )
                        })?
                    }
                };
                x.push(x_v);
                y.push(y_v);
            }
        }
        other => {
            return Err(format!(
                "unsupported JSON element type: {other} — use numbers, [x, y] pairs, or objects"
            ))
        }
    }
    Ok(Series {
        x,
        y,
        line_nos: None,
        rows: Rows::Json { elems: arr },
    })
}

// ---------------------------------------------------------------------------
// Algorithms — each returns sorted, deduplicated 0-based indices to keep.
// ---------------------------------------------------------------------------

/// Largest-Triangle-Three-Buckets — exact port of the canonical
/// flot-downsample bucket math (Steinarsson 2013). Always keeps the first and
/// last point; returns exactly `n_out` indices when the series is longer.
fn lttb_indices(x: &[f64], y: &[f64], n_out: usize) -> Vec<usize> {
    let n = x.len();
    if n_out >= n {
        return (0..n).collect();
    }
    if n_out <= 2 {
        return vec![0, n - 1];
    }
    let every = (n - 2) as f64 / (n_out - 2) as f64;
    let mut out = Vec::with_capacity(n_out);
    let mut a = 0usize;
    out.push(0);
    for i in 0..(n_out - 2) {
        // Average of the NEXT bucket.
        let mut avg_start = ((i as f64 + 1.0) * every).floor() as usize + 1;
        let avg_end = ((((i as f64 + 2.0) * every).floor() as usize) + 1).min(n);
        if avg_start >= avg_end {
            avg_start = avg_end - 1;
        }
        let cnt = (avg_end - avg_start) as f64;
        let (mut avg_x, mut avg_y) = (0.0f64, 0.0f64);
        for j in avg_start..avg_end {
            avg_x += x[j];
            avg_y += y[j];
        }
        avg_x /= cnt;
        avg_y /= cnt;
        // Current bucket range.
        let range_start = (i as f64 * every).floor() as usize + 1;
        let mut range_end = (((i as f64 + 1.0) * every).floor() as usize + 1).min(n - 1);
        if range_end <= range_start {
            range_end = range_start + 1;
        }
        let (pax, pay) = (x[a], y[a]);
        let mut max_area = -1.0f64;
        let mut next_a = range_start;
        for j in range_start..range_end {
            let area = ((pax - avg_x) * (y[j] - pay) - (pax - x[j]) * (avg_y - pay)).abs();
            if area > max_area {
                max_area = area;
                next_a = j;
            }
        }
        out.push(next_a);
        a = next_a;
    }
    out.push(n - 1);
    out
}

/// Min + max per bucket (`n_out / 2` buckets) — keeps every spike envelope.
fn minmax_indices(y: &[f64], n_out: usize) -> Vec<usize> {
    let n = y.len();
    if n_out >= n {
        return (0..n).collect();
    }
    let bins = (n_out / 2).max(1);
    let mut out = Vec::with_capacity(bins * 2);
    for b in 0..bins {
        let s = b * n / bins;
        let e = ((b + 1) * n / bins).max(s + 1);
        let (mut mn, mut mx) = (s, s);
        for j in s..e {
            if y[j] < y[mn] {
                mn = j;
            }
            if y[j] > y[mx] {
                mx = j;
            }
        }
        if mn == mx {
            out.push(mn);
        } else {
            out.push(mn.min(mx));
            out.push(mn.max(mx));
        }
    }
    out
}

/// First/min/max/last per bucket (`n_out / 4` buckets) — the M4 aggregation.
fn m4_indices(y: &[f64], n_out: usize) -> Vec<usize> {
    let n = y.len();
    if n_out >= n {
        return (0..n).collect();
    }
    let bins = (n_out / 4).max(1);
    let mut out = Vec::with_capacity(bins * 4);
    for b in 0..bins {
        let s = b * n / bins;
        let e = ((b + 1) * n / bins).max(s + 1);
        let (mut mn, mut mx) = (s, s);
        for j in s..e {
            if y[j] < y[mn] {
                mn = j;
            }
            if y[j] > y[mx] {
                mx = j;
            }
        }
        let mut cand = [s, mn, mx, e - 1];
        cand.sort_unstable();
        for (k, idx) in cand.iter().enumerate() {
            if k == 0 || cand[k - 1] != *idx {
                out.push(*idx);
            }
        }
    }
    out
}

/// Uniform stride including both endpoints — exactly `n_out` points.
fn nth_indices(n: usize, n_out: usize) -> Vec<usize> {
    if n_out >= n {
        return (0..n).collect();
    }
    let mut out = Vec::with_capacity(n_out);
    for k in 0..n_out {
        let idx = ((k as f64 * (n - 1) as f64 / (n_out - 1) as f64).round() as usize).min(n - 1);
        if out.last() != Some(&idx) {
            out.push(idx);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tests — expected selections cross-checked against an independent Python
// port of the canonical flot-downsample JS.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// 20-point fixture with two spikes (45 @ t=5, 80 @ t=17).
    const V: [f64; 20] = [
        10.0, 12.0, 11.0, 13.0, 45.0, 12.0, 10.0, 11.0, 13.0, 12.0, 14.0, 13.0, 12.0, 15.0, 13.0,
        12.0, 80.0, 13.0, 12.0, 11.0,
    ];

    fn fixture_csv() -> String {
        let mut s = String::from("t,v");
        for (i, v) in V.iter().enumerate() {
            s.push_str(&format!("\n{},{}", i + 1, v));
        }
        s
    }

    #[test]
    fn lttb_csv_keeps_spikes_and_endpoints() {
        // Reference indices for points=8: [0, 3, 4, 7, 10, 15, 16, 19]
        let out = downsample(&fixture_csv(), "lttb", 8, "", "", true, "points").unwrap();
        let expected = "t,v\n1,10\n4,13\n5,45\n8,11\n11,14\n16,12\n17,80\n20,11";
        assert_eq!(out, expected);
    }

    #[test]
    fn lttb_indices_output() {
        let out = downsample(&fixture_csv(), "lttb", 8, "", "", true, "indices").unwrap();
        assert_eq!(out, "[0,3,4,7,10,15,16,19]");
    }

    #[test]
    fn minmax_selection() {
        let out = downsample(&fixture_csv(), "minmax", 8, "", "", true, "indices").unwrap();
        assert_eq!(out, "[0,4,6,8,12,13,16,19]");
    }

    #[test]
    fn minmax_odd_points_rounds_down() {
        let out = downsample(&fixture_csv(), "minmax", 7, "", "", true, "indices").unwrap();
        assert_eq!(out, "[0,4,6,10,16,19]");
    }

    #[test]
    fn m4_selection() {
        let out = downsample(&fixture_csv(), "m4", 8, "", "", true, "indices").unwrap();
        assert_eq!(out, "[0,4,9,10,16,19]");
    }

    #[test]
    fn m4_needs_four_points() {
        let err = downsample(&fixture_csv(), "m4", 3, "", "", true, "points").unwrap_err();
        assert!(err.contains("points >= 4"), "{err}");
    }

    #[test]
    fn nth_selection() {
        let out = downsample(&fixture_csv(), "nth", 8, "", "", true, "indices").unwrap();
        assert_eq!(out, "[0,3,5,8,11,14,16,19]");
    }

    #[test]
    fn points_two_keeps_first_and_last() {
        let out = downsample(&fixture_csv(), "lttb", 2, "", "", true, "points").unwrap();
        assert_eq!(out, "t,v\n1,10\n20,11");
    }

    #[test]
    fn points_at_or_above_length_returns_unchanged() {
        let csv = fixture_csv();
        let out = downsample(&csv, "lttb", 25, "", "", true, "points").unwrap();
        assert_eq!(out, csv);
        let out20 = downsample(&csv, "lttb", 20, "", "", true, "points").unwrap();
        assert_eq!(out20, csv);
    }

    #[test]
    fn single_column_uses_row_index() {
        // Reference (x = 0..19): lttb points=6 → [0, 4, 5, 12, 16, 19]
        let data = V
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let out = downsample(&data, "lttb", 6, "", "", true, "points").unwrap();
        assert_eq!(out, "10\n45\n12\n12\n80\n11");
    }

    #[test]
    fn numeric_first_row_is_data_even_with_header_true() {
        let out = downsample("1,5\n2,6\n3,7\n4,8", "nth", 2, "", "", true, "points").unwrap();
        assert_eq!(out, "1,5\n4,8");
    }

    #[test]
    fn header_false_makes_text_first_row_an_error() {
        let err = downsample("t,v\n1,5\n2,6", "lttb", 2, "", "", false, "points").unwrap_err();
        assert!(err.contains("line 1"), "{err}");
        assert!(err.contains("not a finite number"), "{err}");
    }

    #[test]
    fn extra_columns_come_along_verbatim() {
        let data = "t,v,note\n1,5,\"a, quoted\"\n2,6,plain\n3,7,x\n4,8,y";
        let out = downsample(data, "nth", 2, "", "", true, "points").unwrap();
        assert_eq!(out, "t,v,note\n1,5,\"a, quoted\"\n4,8,y");
    }

    #[test]
    fn y_column_by_name_and_number() {
        let data = "t,temp,humidity\n1,10,99\n2,20,98\n3,30,97\n4,40,96";
        let by_name = downsample(data, "nth", 2, "t", "humidity", true, "indices").unwrap();
        let by_num = downsample(data, "nth", 2, "1", "3", true, "indices").unwrap();
        assert_eq!(by_name, "[0,3]");
        assert_eq!(by_name, by_num);
    }

    #[test]
    fn x_column_index_ignores_columns() {
        let out = downsample("a,5\nb,6\nc,7\nd,8", "nth", 2, "index", "2", true, "points").unwrap();
        // header=true: first row "a,5" has non-numeric 'a' → treated as header.
        assert_eq!(out, "a,5\nb,6\nd,8");
    }

    #[test]
    fn iso_dates_as_x() {
        let data = "date,close\n2024-01-01,10\n2024-01-02,12\n2024-01-03,11\n2024-01-05,45\n2024-01-08,12\n2024-01-09,10";
        let out = downsample(data, "lttb", 4, "", "", true, "points").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 5); // header + 4 points
        assert_eq!(lines[0], "date,close");
        assert_eq!(lines[1], "2024-01-01,10");
        assert!(out.contains("2024-01-05,45"), "spike kept: {out}");
        assert_eq!(*lines.last().unwrap(), "2024-01-09,10");
    }

    #[test]
    fn rfc3339_and_datetime_x_parse() {
        assert!(parse_x("2024-01-02T03:04:05Z").is_some());
        assert!(parse_x("2024-01-02T03:04:05+02:00").is_some());
        assert!(parse_x("2024-01-02 03:04:05").is_some());
        assert!(parse_x("2024-01-02 03:04").is_some());
        assert!(parse_x("2024-01-02").is_some());
        assert!(parse_x("not-a-date").is_none());
    }

    #[test]
    fn unsorted_x_errors_with_row() {
        let err =
            downsample("t,v\n1,5\n3,6\n2,7\n4,8", "lttb", 2, "", "", true, "points").unwrap_err();
        assert!(err.contains("non-decreasing"), "{err}");
        assert!(err.contains("line 4"), "{err}");
    }

    #[test]
    fn tab_delimiter_autodetected() {
        let out =
            downsample("t\tv\n1\t5\n2\t6\n3\t7\n4\t8", "nth", 2, "", "", true, "points").unwrap();
        assert_eq!(out, "t\tv\n1\t5\n4\t8");
    }

    #[test]
    fn semicolon_delimiter_autodetected() {
        let out = downsample("t;v\n1;5\n2;6\n3;7\n4;8", "nth", 2, "", "", true, "indices").unwrap();
        assert_eq!(out, "[0,3]");
    }

    #[test]
    fn json_number_list() {
        let out = downsample("[10, 45, 12, 11, 13, 9]", "lttb", 3, "", "", true, "points").unwrap();
        assert_eq!(out, "[\n  10,\n  45,\n  9\n]");
    }

    #[test]
    fn json_pairs() {
        let out = downsample(
            "[[1,10],[2,45],[3,12],[4,11],[5,13],[6,9]]",
            "lttb",
            3,
            "",
            "",
            true,
            "points",
        )
        .unwrap();
        assert_eq!(out, "[\n  [1,10],\n  [2,45],\n  [6,9]\n]");
    }

    #[test]
    fn json_objects_auto_keys() {
        let data = r#"[{"time":1,"value":10},{"time":2,"value":45},{"time":3,"value":12},{"time":4,"value":9}]"#;
        let out = downsample(data, "lttb", 3, "", "", true, "points").unwrap();
        assert_eq!(
            out,
            "[\n  {\"time\":1,\"value\":10},\n  {\"time\":2,\"value\":45},\n  {\"time\":4,\"value\":9}\n]"
        );
    }

    #[test]
    fn json_objects_named_keys() {
        let data = r#"[{"day":"2024-01-01","close":10},{"day":"2024-01-02","close":45},{"day":"2024-01-03","close":12},{"day":"2024-01-04","close":9}]"#;
        let out = downsample(data, "nth", 2, "day", "close", true, "indices").unwrap();
        assert_eq!(out, "[0,3]");
    }

    #[test]
    fn json_objects_missing_value_key_errors() {
        let err =
            downsample(r#"[{"a":1},{"a":2}]"#, "lttb", 2, "", "", true, "points").unwrap_err();
        assert!(err.contains("y_column"), "{err}");
    }

    #[test]
    fn invalid_json_errors() {
        let err = downsample("[1, 2,", "lttb", 2, "", "", true, "points").unwrap_err();
        assert!(err.contains("invalid JSON"), "{err}");
    }

    #[test]
    fn non_numeric_y_names_the_line() {
        let err =
            downsample("t,v\n1,5\n2,oops\n3,7", "lttb", 2, "", "", true, "points").unwrap_err();
        assert!(err.contains("line 3"), "{err}");
        assert!(err.contains("oops"), "{err}");
    }

    #[test]
    fn empty_input_errors() {
        let err = downsample("   \n  ", "lttb", 10, "", "", true, "points").unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn points_bounds_enforced() {
        let err = downsample("1\n2\n3", "lttb", 1, "", "", true, "points").unwrap_err();
        assert!(err.contains("between 2 and"), "{err}");
        let err = downsample("1\n2\n3", "lttb", 100_001, "", "", true, "points").unwrap_err();
        assert!(err.contains("between 2 and"), "{err}");
    }

    #[test]
    fn byte_cap_at_and_over() {
        // Exactly at the cap: one long valid number line padded with zeros.
        let mut at = String::from("5\n6\n7\n1.");
        at.push_str(&"0".repeat(MAX_BYTES - at.len()));
        assert_eq!(at.len(), MAX_BYTES);
        let out = downsample(&at, "nth", 2, "", "", true, "indices").unwrap();
        assert_eq!(out, "[0,3]");
        // One over: rejected with the cap named.
        let mut over = at;
        over.push('0');
        let err = downsample(&over, "nth", 2, "", "", true, "points").unwrap_err();
        assert!(err.contains("2000000 bytes"), "{err}");
    }

    #[test]
    fn unknown_algorithm_errors() {
        let err = downsample("1\n2\n3", "bogus", 2, "", "", true, "points").unwrap_err();
        assert!(err.contains("unknown algorithm"), "{err}");
    }

    #[test]
    fn same_column_for_x_and_y_errors() {
        let err = downsample("t,v\n1,5\n2,6", "lttb", 2, "t", "t", true, "points").unwrap_err();
        assert!(err.contains("both resolve"), "{err}");
    }

    #[test]
    fn large_series_exact_count() {
        // 10k points → exactly 100 out (lttb), endpoints kept.
        let data = (0..10_000)
            .map(|i| format!("{i},{}", (i as f64 / 50.0).sin() * 100.0))
            .collect::<Vec<_>>()
            .join("\n");
        let out = downsample(&data, "lttb", 100, "", "", true, "points").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 100);
        assert!(lines[0].starts_with("0,"));
        assert!(lines[99].starts_with("9999,"));
    }
}
