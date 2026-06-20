//! csv-json-convert core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps. Converts tabular data between CSV and
//! JSON in either direction, with auto-detection of the input format.

use serde_json::{Map, Value};

/// Conversion direction. `auto` sniffs the input: a payload whose first
/// non-space char is `[` or `{` is treated as JSON (→ CSV), else CSV (→ JSON).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Auto,
    CsvToJson,
    JsonToCsv,
}

impl Direction {
    pub fn parse(s: &str) -> Result<Direction, String> {
        match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "" | "auto" => Ok(Direction::Auto),
            "csv-to-json" | "csv2json" | "csv" => Ok(Direction::CsvToJson),
            "json-to-csv" | "json2csv" | "json" => Ok(Direction::JsonToCsv),
            other => Err(format!(
                "unknown direction '{other}' (use auto, csv-to-json, or json-to-csv)"
            )),
        }
    }
}

/// Resolve a delimiter string to a single byte. Accepts a literal char or the
/// words `tab` / `comma` / `semicolon` / `pipe` for convenience.
fn delimiter_byte(delimiter: &str) -> Result<u8, String> {
    let d = match delimiter {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let bytes = other.as_bytes();
            if bytes.len() == 1 {
                bytes[0]
            } else {
                return Err(format!(
                    "delimiter must be a single character (or tab/comma/semicolon/pipe), got '{other}'"
                ));
            }
        }
    };
    Ok(d)
}

/// Convert CSV/JSON. `headers` controls whether a CSV's first row is treated as
/// field names (csv→json: array of objects vs array of arrays; json→csv: emit a
/// header row from object keys). `pretty` pretty-prints JSON output. `flatten`
/// (json→csv only) expands nested objects/arrays into dot-notation columns
/// (`{"a":{"b":1}}` → column `a.b`) instead of writing them as JSON strings.
pub fn convert(
    data: &str,
    direction: Direction,
    delimiter: &str,
    headers: bool,
    pretty: bool,
    flatten: bool,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let dir = match direction {
        Direction::Auto => {
            let first = data.trim_start().chars().next().unwrap_or(' ');
            if first == '[' || first == '{' {
                Direction::JsonToCsv
            } else {
                Direction::CsvToJson
            }
        }
        d => d,
    };
    let delim = delimiter_byte(delimiter)?;
    match dir {
        Direction::CsvToJson => csv_to_json(data, delim, headers, pretty),
        Direction::JsonToCsv => json_to_csv(data, delim, headers, flatten),
        Direction::Auto => unreachable!(),
    }
}

fn csv_to_json(data: &str, delim: u8, headers: bool, pretty: bool) -> Result<String, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(headers)
        .flexible(true)
        .from_reader(data.as_bytes());

    let mut out: Vec<Value> = Vec::new();
    if headers {
        let hdr = rdr
            .headers()
            .map_err(|e| format!("CSV parse error: {e}"))?
            .clone();
        for rec in rdr.records() {
            let rec = rec.map_err(|e| format!("CSV parse error: {e}"))?;
            let mut obj = Map::new();
            for (i, field) in rec.iter().enumerate() {
                let key = hdr.get(i).unwrap_or("").to_string();
                obj.insert(key, infer_scalar(field));
            }
            out.push(Value::Object(obj));
        }
    } else {
        for rec in rdr.records() {
            let rec = rec.map_err(|e| format!("CSV parse error: {e}"))?;
            let row: Vec<Value> = rec.iter().map(infer_scalar).collect();
            out.push(Value::Array(row));
        }
    }
    let val = Value::Array(out);
    if pretty {
        serde_json::to_string_pretty(&val)
    } else {
        serde_json::to_string(&val)
    }
    .map_err(|e| format!("JSON encode error: {e}"))
}

/// Infer a JSON scalar from a CSV cell: integers, floats, booleans, and empty →
/// null; everything else stays a string. Leading-zero / `+`-prefixed strings
/// (e.g. zip codes, phone numbers) are kept as strings, not coerced to numbers.
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
        if f.is_finite() && format_finite(f) == s {
            return Value::from(f);
        }
    }
    Value::String(s.to_string())
}

fn format_finite(f: f64) -> String {
    // serde_json renders 2.0 as "2.0"; only treat as a number if the textual
    // form round-trips exactly, so "2.0" stays a number but "2." stays a string.
    let v = Value::from(f);
    v.to_string()
}

fn json_to_csv(data: &str, delim: u8, headers: bool, flatten: bool) -> Result<String, String> {
    let parsed: Value = serde_json::from_str(data).map_err(|e| format!("JSON parse error: {e}"))?;
    // Accept a top-level array, or a single object (wrapped into a 1-row table).
    let mut rows: Vec<Value> = match parsed {
        Value::Array(a) => a,
        obj @ Value::Object(_) => vec![obj],
        _ => return Err("JSON must be an array of rows (or a single object)".into()),
    };
    if rows.is_empty() {
        return Ok(String::new());
    }
    // Flatten each object row to dot-notation keys before tabulating.
    if flatten {
        rows = rows
            .into_iter()
            .map(|r| match r {
                Value::Object(_) => {
                    let mut flat = Map::new();
                    flatten_into(&mut flat, "", &r);
                    Value::Object(flat)
                }
                other => other,
            })
            .collect();
    }

    let mut wtr = csv::WriterBuilder::new().delimiter(delim).from_writer(vec![]);

    let all_objects = rows.iter().all(|r| r.is_object());
    if all_objects {
        // Union of keys in first-seen order across all rows.
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
                .map_err(|e| format!("CSV write error: {e}"))?;
        }
        for r in &rows {
            let m = r.as_object().unwrap();
            let rec: Vec<String> = keys
                .iter()
                .map(|k| m.get(k).map(cell_to_string).unwrap_or_default())
                .collect();
            wtr.write_record(&rec)
                .map_err(|e| format!("CSV write error: {e}"))?;
        }
    } else {
        // Array of arrays (or scalars) → write each row positionally.
        for r in &rows {
            let rec: Vec<String> = match r {
                Value::Array(a) => a.iter().map(cell_to_string).collect(),
                other => vec![cell_to_string(other)],
            };
            wtr.write_record(&rec)
                .map_err(|e| format!("CSV write error: {e}"))?;
        }
    }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("CSV utf8 error: {e}"))
}

/// Recursively flatten a JSON value into `out` with dot-notation keys. Objects
/// recurse by key (`a.b`), arrays by index (`a.0`), scalars are inserted as-is.
/// An empty object/array flattens to its own empty-string cell so the column
/// still appears.
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
        // scalar, or empty object/array → leaf
        leaf => {
            out.insert(prefix.to_string(), leaf.clone());
        }
    }
}

/// Stringify a JSON value for a CSV cell. Scalars become their plain text;
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

    #[test]
    fn csv_to_json_objects_with_type_inference() {
        let out = convert(
            "name,age,active\nAlice,30,true\nBob,25,false",
            Direction::CsvToJson,
            ",",
            true,
            false,
            false,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["name"], "Alice");
        assert_eq!(v[0]["age"], 30);
        assert_eq!(v[0]["active"], true);
        assert_eq!(v[1]["age"], 25);
    }

    #[test]
    fn csv_to_json_no_headers_is_arrays() {
        let out = convert("a,b\nc,d", Direction::CsvToJson, ",", false, false, false).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v, serde_json::json!([["a", "b"], ["c", "d"]]));
    }

    #[test]
    fn json_to_csv_objects_unions_keys() {
        let out = convert(
            r#"[{"a":1,"b":2},{"a":3,"c":4}]"#,
            Direction::JsonToCsv,
            ",",
            true,
            false,
            false,
        )
        .unwrap();
        assert_eq!(out, "a,b,c\n1,2,\n3,,4\n");
    }

    #[test]
    fn json_to_csv_flatten_dot_notation() {
        let out = convert(
            r#"[{"name":"Al","addr":{"city":"NYC","zip":"10001"}}]"#,
            Direction::JsonToCsv,
            ",",
            true,
            false,
            true,
        )
        .unwrap();
        assert_eq!(out, "name,addr.city,addr.zip\nAl,NYC,10001\n");
    }

    #[test]
    fn json_to_csv_without_flatten_keeps_nested_json() {
        let out = convert(
            r#"[{"addr":{"city":"NYC"}}]"#,
            Direction::JsonToCsv,
            ",",
            true,
            false,
            false,
        )
        .unwrap();
        assert_eq!(out, "addr\n\"{\"\"city\"\":\"\"NYC\"\"}\"\n");
    }

    #[test]
    fn auto_detects_json_input() {
        let out = convert(r#"[{"x":"1"}]"#, Direction::Auto, ",", true, false, false).unwrap();
        assert_eq!(out, "x\n1\n");
    }

    #[test]
    fn auto_detects_csv_input() {
        let out = convert("x\n1", Direction::Auto, ",", true, false, false).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["x"], 1);
    }

    #[test]
    fn round_trip_preserves_quoted_commas() {
        let csv = "name,note\nAlice,\"hi, there\"";
        let json = convert(csv, Direction::CsvToJson, ",", true, false, false).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v[0]["note"], "hi, there");
        // and back to CSV re-quotes the embedded comma
        let back = convert(&json, Direction::JsonToCsv, ",", true, false, false).unwrap();
        assert_eq!(back, "name,note\nAlice,\"hi, there\"\n");
    }

    #[test]
    fn tab_delimiter_word() {
        let out = convert("a\tb\n1\t2", Direction::CsvToJson, "tab", true, false, false).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["a"], 1);
        assert_eq!(v[0]["b"], 2);
    }

    #[test]
    fn leading_zero_stays_string() {
        let out = convert("zip\n007", Direction::CsvToJson, ",", true, false, false).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["zip"], "007");
    }

    #[test]
    fn empty_input_errors() {
        assert!(convert("   ", Direction::Auto, ",", true, false, false).is_err());
    }

    #[test]
    fn bad_json_errors() {
        assert!(convert("{not json", Direction::JsonToCsv, ",", true, false, false).is_err());
    }

    #[test]
    fn bad_delimiter_errors() {
        assert!(convert("a,b\n1,2", Direction::CsvToJson, "xx", true, false, false).is_err());
    }
}
