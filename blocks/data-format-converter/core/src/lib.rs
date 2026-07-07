//! data-format-converter core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps. Converts tabular / record data
//! between CSV, TSV, JSON (array) and NDJSON (newline-delimited JSON, a.k.a.
//! JSONL) in any direction, via a common intermediate representation: a list of
//! records (each an object, or an array for header-less rows).

use serde_json::{Map, Value};

/// A supported data-interchange format.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    /// Detect the source format from the input text. Source side only —
    /// [`convert`] rejects it as a target.
    Auto,
    /// Comma-separated values.
    Csv,
    /// Tab-separated values.
    Tsv,
    /// A single JSON array of records (a top-level object is accepted as one row).
    Json,
    /// Newline-delimited JSON — one JSON value per line (a.k.a. JSONL / JSON Lines).
    Ndjson,
}

impl Format {
    /// Parse a format name. Accepts the canonical `auto|csv|tsv|json|ndjson`
    /// plus the common aliases `tab`, `jsonl`, `json-lines`, `jsonlines`.
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Format::Auto),
            "csv" => Ok(Format::Csv),
            "tsv" | "tab" => Ok(Format::Tsv),
            "json" => Ok(Format::Json),
            "ndjson" | "jsonl" | "json-lines" | "jsonlines" => Ok(Format::Ndjson),
            other => Err(format!(
                "unknown format '{other}' (use auto, csv, tsv, json, or ndjson)"
            )),
        }
    }

    /// The field separator byte for the delimited formats.
    fn delimiter(self) -> Option<u8> {
        match self {
            Format::Csv => Some(b','),
            Format::Tsv => Some(b'\t'),
            _ => None,
        }
    }
}

/// Sniff the source format of `data` for [`Format::Auto`]:
/// - first non-space char `[` → a JSON array;
/// - first non-space char `{` → a single JSON object, unless there are 2+
///   non-blank lines that each independently parse as JSON (then NDJSON);
/// - otherwise delimited text: TSV when the first row has a tab and at least as
///   many tabs as commas, else CSV.
fn detect(data: &str) -> Format {
    let first = data.trim_start().chars().next().unwrap_or(' ');
    if first == '[' {
        Format::Json
    } else if first == '{' {
        let lines: Vec<&str> = data
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        if lines.len() >= 2 && lines.iter().all(|l| serde_json::from_str::<Value>(l).is_ok()) {
            Format::Ndjson
        } else {
            Format::Json
        }
    } else {
        let first_line = data.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        let tabs = first_line.matches('\t').count();
        let commas = first_line.matches(',').count();
        if tabs > 0 && tabs >= commas {
            Format::Tsv
        } else {
            Format::Csv
        }
    }
}

/// Convert `data` from `from` to `to`.
///
/// - `headers` — treat a delimited file's first row as field names (parsing:
///   objects vs arrays of arrays; writing: emit a header row from the keys).
/// - `infer_types` — coerce CSV/TSV cells to JSON numbers/booleans/null; when
///   false every cell stays a string. Ignored for JSON/NDJSON sources (they are
///   already typed).
/// - `pretty` — indent JSON-array output (no effect on NDJSON/CSV/TSV; NDJSON is
///   always one compact record per line).
/// - `flatten` — when writing CSV/TSV, expand nested objects/arrays into
///   dot-notation columns (`{"a":{"b":1}}` → column `a.b`) instead of writing
///   them as compact JSON strings.
pub fn convert(
    data: &str,
    from: Format,
    to: Format,
    headers: bool,
    infer_types: bool,
    pretty: bool,
    flatten: bool,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    if to == Format::Auto {
        return Err("target format cannot be 'auto' — choose csv, tsv, json, or ndjson".into());
    }
    let src = match from {
        Format::Auto => detect(data),
        f => f,
    };
    let records = parse_records(data, src, headers, infer_types)?;
    serialize_records(&records, to, headers, pretty, flatten)
}

/// Parse `data` (in the concrete `fmt`, never [`Format::Auto`]) into a list of
/// record `Value`s.
fn parse_records(
    data: &str,
    fmt: Format,
    headers: bool,
    infer_types: bool,
) -> Result<Vec<Value>, String> {
    match fmt {
        Format::Csv | Format::Tsv => {
            parse_delimited(data, fmt.delimiter().unwrap(), headers, infer_types)
        }
        Format::Json => {
            let parsed: Value =
                serde_json::from_str(data).map_err(|e| format!("JSON parse error: {e}"))?;
            match parsed {
                Value::Array(a) => Ok(a),
                obj @ Value::Object(_) => Ok(vec![obj]),
                _ => Err("JSON input must be an array of records (or a single object)".into()),
            }
        }
        Format::Ndjson => {
            let mut out = Vec::new();
            for (i, line) in data.lines().enumerate() {
                let t = line.trim();
                if t.is_empty() {
                    continue;
                }
                let v: Value = serde_json::from_str(t)
                    .map_err(|e| format!("NDJSON parse error on line {}: {e}", i + 1))?;
                out.push(v);
            }
            if out.is_empty() {
                return Err("NDJSON input has no records".into());
            }
            Ok(out)
        }
        Format::Auto => unreachable!("source format is resolved before parse_records"),
    }
}

fn parse_delimited(
    data: &str,
    delim: u8,
    headers: bool,
    infer_types: bool,
) -> Result<Vec<Value>, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(headers)
        .flexible(true)
        .from_reader(data.as_bytes());

    let mut out: Vec<Value> = Vec::new();
    if headers {
        let hdr = rdr
            .headers()
            .map_err(|e| format!("CSV/TSV parse error: {e}"))?
            .clone();
        for rec in rdr.records() {
            let rec = rec.map_err(|e| format!("CSV/TSV parse error: {e}"))?;
            let mut obj = Map::new();
            for (i, field) in rec.iter().enumerate() {
                let key = hdr.get(i).unwrap_or("").to_string();
                obj.insert(key, cell_value(field, infer_types));
            }
            out.push(Value::Object(obj));
        }
    } else {
        for rec in rdr.records() {
            let rec = rec.map_err(|e| format!("CSV/TSV parse error: {e}"))?;
            let row: Vec<Value> = rec.iter().map(|f| cell_value(f, infer_types)).collect();
            out.push(Value::Array(row));
        }
    }
    Ok(out)
}

/// Turn one delimited cell into a JSON value. With `infer=false` every cell
/// stays a string (empty → `""`); with `infer=true` see [`infer_scalar`].
fn cell_value(s: &str, infer: bool) -> Value {
    if infer {
        infer_scalar(s)
    } else {
        Value::String(s.to_string())
    }
}

/// Infer a JSON scalar from a delimited cell: integers, floats, booleans, and
/// empty → null; everything else stays a string. Leading-zero / `+`-prefixed
/// strings (zip codes, phone numbers) are kept as strings, not coerced.
fn infer_scalar(s: &str) -> Value {
    if s.is_empty() {
        return Value::Null;
    }
    match s {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    if let Ok(i) = s.parse::<i64>() {
        // Reject re-serialization mismatches (e.g. "007", "+1", "1_000").
        if i.to_string() == s {
            return Value::from(i);
        }
    }
    if let Ok(f) = s.parse::<f64>() {
        if f.is_finite() && Value::from(f).to_string() == s {
            return Value::from(f);
        }
    }
    Value::String(s.to_string())
}

/// Serialize a list of records into the concrete target `fmt`.
fn serialize_records(
    records: &[Value],
    fmt: Format,
    headers: bool,
    pretty: bool,
    flatten: bool,
) -> Result<String, String> {
    match fmt {
        Format::Json => {
            let val = Value::Array(records.to_vec());
            if pretty {
                serde_json::to_string_pretty(&val)
            } else {
                serde_json::to_string(&val)
            }
            .map_err(|e| format!("JSON encode error: {e}"))
        }
        Format::Ndjson => {
            let mut lines = Vec::with_capacity(records.len());
            for r in records {
                lines.push(serde_json::to_string(r).map_err(|e| format!("JSON encode error: {e}"))?);
            }
            Ok(lines.join("\n"))
        }
        Format::Csv | Format::Tsv => {
            records_to_delimited(records, fmt.delimiter().unwrap(), headers, flatten)
        }
        Format::Auto => Err("target format cannot be 'auto'".into()),
    }
}

fn records_to_delimited(
    records: &[Value],
    delim: u8,
    headers: bool,
    flatten: bool,
) -> Result<String, String> {
    if records.is_empty() {
        return Ok(String::new());
    }
    // Optionally flatten object rows to dot-notation keys before tabulating.
    let rows: Vec<Value> = if flatten {
        records
            .iter()
            .map(|r| match r {
                Value::Object(_) => {
                    let mut flat = Map::new();
                    flatten_into(&mut flat, "", r);
                    Value::Object(flat)
                }
                other => other.clone(),
            })
            .collect()
    } else {
        records.to_vec()
    };

    let mut wtr = csv::WriterBuilder::new().delimiter(delim).from_writer(vec![]);
    let all_objects = rows.iter().all(|r| r.is_object());
    if all_objects {
        // Union of keys across all rows, in first-seen order.
        let mut keys: Vec<String> = Vec::new();
        for r in &rows {
            if let Value::Object(m) = r {
                for k in m.keys() {
                    if !keys.iter().any(|e| e == k) {
                        keys.push(k.clone());
                    }
                }
            }
        }
        if headers {
            wtr.write_record(&keys)
                .map_err(|e| format!("CSV/TSV write error: {e}"))?;
        }
        for r in &rows {
            let m = r.as_object().unwrap();
            let rec: Vec<String> = keys
                .iter()
                .map(|k| m.get(k).map(cell_to_string).unwrap_or_default())
                .collect();
            wtr.write_record(&rec)
                .map_err(|e| format!("CSV/TSV write error: {e}"))?;
        }
    } else {
        // Array-of-arrays (or scalars) → write each row positionally.
        for r in &rows {
            let rec: Vec<String> = match r {
                Value::Array(a) => a.iter().map(cell_to_string).collect(),
                other => vec![cell_to_string(other)],
            };
            wtr.write_record(&rec)
                .map_err(|e| format!("CSV/TSV write error: {e}"))?;
        }
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV/TSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("CSV/TSV utf8 error: {e}"))
}

/// Recursively flatten a JSON value into `out` with dot-notation keys. Objects
/// recurse by key (`a.b`), arrays by index (`a.0`), scalars are inserted as-is.
/// An empty object/array flattens to its own empty cell so the column appears.
fn flatten_into(out: &mut Map<String, Value>, prefix: &str, v: &Value) {
    match v {
        Value::Object(m) if !m.is_empty() => {
            for (k, val) in m {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten_into(out, &key, val);
            }
        }
        Value::Array(a) if !a.is_empty() => {
            for (i, val) in a.iter().enumerate() {
                let key = if prefix.is_empty() {
                    i.to_string()
                } else {
                    format!("{prefix}.{i}")
                };
                flatten_into(out, &key, val);
            }
        }
        leaf => {
            out.insert(prefix.to_string(), leaf.clone());
        }
    }
}

/// Stringify a JSON value for a delimited cell. Scalars become their plain text;
/// nested arrays/objects become compact JSON so no data is silently dropped.
fn cell_to_string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(data: &str, from: &str, to: &str, headers: bool, infer: bool, pretty: bool, flat: bool) -> String {
        convert(
            data,
            Format::parse(from).unwrap(),
            Format::parse(to).unwrap(),
            headers,
            infer,
            pretty,
            flat,
        )
        .unwrap()
    }

    #[test]
    fn csv_to_json_objects_with_type_inference() {
        let out = conv("name,age,active\nAlice,30,true\nBob,25,false", "csv", "json", true, true, false, false);
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["name"], "Alice");
        assert_eq!(v[0]["age"], 30);
        assert_eq!(v[0]["active"], true);
        assert_eq!(v[1]["age"], 25);
    }

    #[test]
    fn tsv_to_json() {
        let out = conv("a\tb\n1\t2", "tsv", "json", true, true, false, false);
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["a"], 1);
        assert_eq!(v[0]["b"], 2);
    }

    #[test]
    fn json_array_to_ndjson() {
        let out = conv(r#"[{"a":1},{"a":2}]"#, "json", "ndjson", true, true, false, false);
        assert_eq!(out, "{\"a\":1}\n{\"a\":2}");
    }

    #[test]
    fn ndjson_to_json_array() {
        let out = conv("{\"a\":1}\n{\"a\":2}", "ndjson", "json", true, true, false, false);
        assert_eq!(out, r#"[{"a":1},{"a":2}]"#);
    }

    #[test]
    fn csv_to_ndjson() {
        let out = conv("name,age\nAlice,30\nBob,25", "csv", "ndjson", true, true, false, false);
        assert_eq!(out, "{\"name\":\"Alice\",\"age\":30}\n{\"name\":\"Bob\",\"age\":25}");
    }

    #[test]
    fn ndjson_to_csv_unions_keys() {
        let out = conv("{\"a\":1,\"b\":2}\n{\"a\":3,\"c\":4}", "ndjson", "csv", true, true, false, false);
        assert_eq!(out, "a,b,c\n1,2,\n3,,4\n");
    }

    #[test]
    fn json_to_tsv() {
        let out = conv(r#"[{"a":1,"b":2}]"#, "json", "tsv", true, true, false, false);
        assert_eq!(out, "a\tb\n1\t2\n");
    }

    #[test]
    fn csv_to_json_pretty() {
        let out = conv("x\n1", "csv", "json", true, true, true, false);
        assert_eq!(out, "[\n  {\n    \"x\": 1\n  }\n]");
    }

    #[test]
    fn infer_types_off_keeps_strings() {
        let out = conv("age,flag\n30,true", "csv", "json", true, false, false, false);
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["age"], "30");
        assert_eq!(v[0]["flag"], "true");
    }

    #[test]
    fn json_to_csv_flatten_dot_notation() {
        let out = conv(r#"[{"name":"Al","addr":{"city":"NYC","zip":"10001"}}]"#, "json", "csv", true, true, false, true);
        assert_eq!(out, "name,addr.city,addr.zip\nAl,NYC,10001\n");
    }

    #[test]
    fn json_to_csv_without_flatten_keeps_nested_json() {
        let out = conv(r#"[{"addr":{"city":"NYC"}}]"#, "json", "csv", true, true, false, false);
        assert_eq!(out, "addr\n\"{\"\"city\"\":\"\"NYC\"\"}\"\n");
    }

    #[test]
    fn auto_detects_ndjson() {
        // from=auto, two standalone JSON lines → NDJSON source.
        let out = conv("{\"x\":1}\n{\"x\":2}", "auto", "csv", true, true, false, false);
        assert_eq!(out, "x\n1\n2\n");
    }

    #[test]
    fn auto_detects_json_array() {
        let out = conv(r#"[{"x":"1"}]"#, "auto", "csv", true, true, false, false);
        assert_eq!(out, "x\n1\n");
    }

    #[test]
    fn auto_detects_tsv() {
        let out = conv("a\tb\n1\t2", "auto", "json", true, true, false, false);
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["a"], 1);
    }

    #[test]
    fn auto_detects_csv() {
        let out = conv("a,b\n1,2", "auto", "json", true, true, false, false);
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["a"], 1);
    }

    #[test]
    fn headers_off_gives_arrays() {
        let out = conv("a,b\nc,d", "csv", "json", false, true, false, false);
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v, serde_json::json!([["a", "b"], ["c", "d"]]));
    }

    #[test]
    fn round_trip_preserves_quoted_commas() {
        let csv = "name,note\nAlice,\"hi, there\"";
        let json = conv(csv, "csv", "json", true, true, false, false);
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v[0]["note"], "hi, there");
        let back = conv(&json, "json", "csv", true, true, false, false);
        assert_eq!(back, "name,note\nAlice,\"hi, there\"\n");
    }

    #[test]
    fn leading_zero_stays_string() {
        let out = conv("zip\n007", "csv", "json", true, true, false, false);
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["zip"], "007");
    }

    #[test]
    fn empty_input_errors() {
        assert!(convert("   ", Format::Auto, Format::Json, true, true, false, false).is_err());
    }

    #[test]
    fn target_auto_errors() {
        assert!(convert("a\n1", Format::Csv, Format::Auto, true, true, false, false).is_err());
    }

    #[test]
    fn bad_json_errors() {
        assert!(convert("{not json", Format::Json, Format::Csv, true, true, false, false).is_err());
    }

    #[test]
    fn bad_ndjson_line_errors() {
        let e = convert("{\"a\":1}\nnope", Format::Ndjson, Format::Json, true, true, false, false)
            .unwrap_err();
        assert!(e.contains("line 2"), "error should name the bad line: {e}");
    }

    #[test]
    fn unknown_format_errors() {
        assert!(Format::parse("xml").is_err());
    }
}
