//! gizza-ai/yaml-to-csv core — flatten a YAML list of records (or a mapping of
//! records) into a CSV table with a unioned column header. Pure-Rust (`serde_yml`
//! to parse + `serde_json` `preserve_order` as the intermediate record model +
//! `csv` to write). No wafer/wasm-bindgen deps.
//!
//! Column order is the first-seen order of keys across every row; nested mappings
//! flatten to dot-paths (`user.name`); arrays render per `array_mode`.

use serde_json::{Map, Value};

/// How array-valued fields are rendered into the flat table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ArrayMode {
    /// The whole array becomes one compact-JSON string in a single cell (default).
    Json,
    /// Scalar arrays are joined by `", "` in one cell; arrays that contain
    /// nested objects/arrays fall back to a compact-JSON string.
    Joined,
    /// Each element becomes its own dot-indexed column (`tags.0`, `tags.1`, …).
    Columns,
}

impl ArrayMode {
    fn parse(s: &str) -> Result<ArrayMode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "json" => Ok(ArrayMode::Json),
            "joined" => Ok(ArrayMode::Joined),
            "columns" => Ok(ArrayMode::Columns),
            other => Err(format!(
                "unknown array_mode '{other}' (use json, joined, or columns)"
            )),
        }
    }
}

/// Resolve a delimiter name/char to the byte the CSV writer uses.
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

/// Flatten a YAML document into CSV.
///
/// - `data` — YAML text: a top-level list of records, or a top-level mapping
///   whose values are records (→ one row each, key kept) or all scalars (→ one row).
/// - `delimiter` — `comma`/`tab`/`semicolon`/`pipe` or a single char (default comma).
/// - `header` — emit the column-name header row (default true).
/// - `array_mode` — `json` | `joined` | `columns` (see [`ArrayMode`]).
/// - `quote_all` — always quote every field, not just those that need it.
/// - `key_column` — for a top-level mapping of records, the header of the column
///   holding each entry's key; blank omits it.
pub fn to_csv(
    data: &str,
    delimiter: &str,
    header: bool,
    array_mode: &str,
    quote_all: bool,
    key_column: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty — paste a YAML list or mapping of records".into());
    }
    let delim = delim_byte(delimiter)?;
    let amode = ArrayMode::parse(array_mode)?;
    let key_col = key_column.trim();

    let yaml: serde_yml::Value =
        serde_yml::from_str(data).map_err(|e| format!("YAML parse error: {e}"))?;
    let root = yaml_to_json(&yaml)?;

    // Resolve the top-level shape into (optional entry-key, record) pairs.
    let records: Vec<(Option<String>, Value)> = match root {
        Value::Array(items) => {
            if items.is_empty() {
                return Err("YAML list is empty — no records to convert".into());
            }
            items.into_iter().map(|v| (None, v)).collect()
        }
        Value::Object(map) => {
            if map.is_empty() {
                return Err("YAML mapping is empty — no records to convert".into());
            }
            if map.values().all(is_scalar) {
                // A single record expressed as a top-level mapping → one row.
                vec![(None, Value::Object(map))]
            } else {
                // Mapping of records → one row per entry, key kept as a column.
                map.into_iter().map(|(k, v)| (Some(k), v)).collect()
            }
        }
        Value::Null => {
            return Err("no YAML content found (the document is empty or only comments)".into());
        }
        other => {
            return Err(format!(
                "unsupported top-level YAML shape: expected a list of records or a mapping of records, got {}",
                shape_name(&other)
            ));
        }
    };

    // Flatten every record into an ordered column map.
    let mut rows: Vec<Map<String, Value>> = Vec::with_capacity(records.len());
    for (key, rec) in records {
        let mut flat: Map<String, Value> = Map::new();
        if let Some(k) = &key {
            if !key_col.is_empty() {
                flat.insert(key_col.to_string(), Value::String(k.clone()));
            }
        }
        match rec {
            // Object records expand their fields at the top level (no prefix).
            Value::Object(_) => flatten_into(&mut flat, "", &rec, amode),
            // A scalar or array record goes under a single "value" column.
            other => flatten_into(&mut flat, "value", &other, amode),
        }
        rows.push(flat);
    }

    // Union of column names across all rows, in first-seen order.
    let mut columns: Vec<String> = Vec::new();
    for row in &rows {
        for k in row.keys() {
            if !columns.iter().any(|c| c == k) {
                columns.push(k.clone());
            }
        }
    }

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .quote_style(if quote_all {
            csv::QuoteStyle::Always
        } else {
            csv::QuoteStyle::Necessary
        })
        .from_writer(vec![]);

    if header {
        wtr.write_record(&columns)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    for row in &rows {
        let rec: Vec<String> = columns
            .iter()
            .map(|c| row.get(c).map(cell_to_string).unwrap_or_default())
            .collect();
        wtr.write_record(&rec)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("CSV utf8 error: {e}"))
}

/// Recursively flatten `v` into `out` under dot-notation `prefix`. Objects
/// recurse by key (`a.b`); arrays follow `amode`; scalars land as-is. Empty
/// containers land as an empty cell so the column still appears.
fn flatten_into(out: &mut Map<String, Value>, prefix: &str, v: &Value, amode: ArrayMode) {
    match v {
        Value::Object(m) if !m.is_empty() => {
            for (k, val) in m {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten_into(out, &key, val, amode);
            }
        }
        Value::Array(a) if !a.is_empty() => match amode {
            ArrayMode::Columns => {
                for (i, val) in a.iter().enumerate() {
                    let key = if prefix.is_empty() {
                        i.to_string()
                    } else {
                        format!("{prefix}.{i}")
                    };
                    flatten_into(out, &key, val, amode);
                }
            }
            ArrayMode::Joined => {
                let cell = if a.iter().all(is_scalar) {
                    Value::String(a.iter().map(cell_to_string).collect::<Vec<_>>().join(", "))
                } else {
                    Value::String(compact_json(v))
                };
                out.insert(col_or_value(prefix), cell);
            }
            ArrayMode::Json => {
                out.insert(col_or_value(prefix), Value::String(compact_json(v)));
            }
        },
        // Scalars, plus any empty object/array (→ empty cell).
        leaf => {
            let cell = match leaf {
                Value::Object(_) | Value::Array(_) => Value::String(String::new()),
                other => other.clone(),
            };
            out.insert(col_or_value(prefix), cell);
        }
    }
}

/// Column name for a leaf; a record that is itself a scalar/array (empty prefix)
/// lands under `value`.
fn col_or_value(prefix: &str) -> String {
    if prefix.is_empty() {
        "value".to_string()
    } else {
        prefix.to_string()
    }
}

fn is_scalar(v: &Value) -> bool {
    !matches!(v, Value::Object(_) | Value::Array(_))
}

fn compact_json(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

/// Stringify a JSON value for a CSV cell. Nulls become empty; scalars become
/// their plain text; any residual container becomes compact JSON.
fn cell_to_string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

fn shape_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "a list",
        Value::Object(_) => "a mapping",
    }
}

/// Convert a parsed YAML value into the `serde_json` intermediate model.
/// Mapping keys are stringified (YAML allows `1:` / `true:` scalar keys);
/// a non-scalar key (a list/map used as a key) is rejected with a clear error.
fn yaml_to_json(v: &serde_yml::Value) -> Result<Value, String> {
    use serde_yml::Value as Y;
    Ok(match v {
        Y::Null => Value::Null,
        Y::Bool(b) => Value::Bool(*b),
        Y::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::from(i)
            } else if let Some(u) = n.as_u64() {
                Value::from(u)
            } else if let Some(f) = n.as_f64() {
                serde_json::Number::from_f64(f)
                    .map(Value::Number)
                    // NaN/Infinity have no JSON form → keep the text.
                    .unwrap_or_else(|| Value::String(f.to_string()))
            } else {
                Value::Null
            }
        }
        Y::String(s) => Value::String(s.clone()),
        Y::Sequence(seq) => {
            let mut out = Vec::with_capacity(seq.len());
            for item in seq {
                out.push(yaml_to_json(item)?);
            }
            Value::Array(out)
        }
        Y::Mapping(m) => {
            let mut out = Map::new();
            for (k, val) in m {
                out.insert(yaml_key_to_string(k)?, yaml_to_json(val)?);
            }
            Value::Object(out)
        }
        // `!Tag value` — drop the tag, keep the value.
        Y::Tagged(t) => yaml_to_json(&t.value)?,
    })
}

fn yaml_key_to_string(k: &serde_yml::Value) -> Result<String, String> {
    use serde_yml::Value as Y;
    Ok(match k {
        Y::String(s) => s.clone(),
        Y::Bool(b) => b.to_string(),
        Y::Number(n) => n.to_string(),
        Y::Null => "null".to_string(),
        _ => return Err("unsupported YAML mapping key: keys must be scalars".into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(data: &str) -> String {
        to_csv(data, "comma", true, "json", false, "key").unwrap()
    }

    #[test]
    fn list_of_records_unions_and_flattens() {
        let yaml = "\
- name: Ada
  address:
    city: London
    zip: N1
- name: Bo
  age: 40
";
        // First-seen union: name, address.city, address.zip, age.
        assert_eq!(
            conv(yaml),
            "name,address.city,address.zip,age\nAda,London,N1,\nBo,,,40\n"
        );
    }

    #[test]
    fn mapping_of_records_keeps_the_key_column() {
        let yaml = "\
alice:
  age: 30
bob:
  age: 40
";
        assert_eq!(conv(yaml), "key,age\nalice,30\nbob,40\n");
    }

    #[test]
    fn top_level_mapping_of_scalars_is_one_record() {
        // Every value scalar → the mapping IS a single record (one row, no key col).
        assert_eq!(conv("name: Ada\nage: 30\n"), "name,age\nAda,30\n");
    }

    #[test]
    fn array_json_mode_is_the_default() {
        assert_eq!(conv("- tags: [a, b]\n"), "tags\n\"[\"\"a\"\",\"\"b\"\"]\"\n");
    }

    #[test]
    fn array_joined_mode() {
        let out = to_csv("- tags: [a, b, c]\n", "comma", true, "joined", false, "key").unwrap();
        assert_eq!(out, "tags\n\"a, b, c\"\n");
    }

    #[test]
    fn array_columns_mode_expands_indices() {
        let out = to_csv("- tags: [a, b]\n", "comma", true, "columns", false, "key").unwrap();
        assert_eq!(out, "tags.0,tags.1\na,b\n");
    }

    #[test]
    fn semicolon_delimiter_and_no_header() {
        let out = to_csv("- a: 1\n  b: 2\n", "semicolon", false, "json", false, "key").unwrap();
        assert_eq!(out, "1;2\n");
    }

    #[test]
    fn tab_delimiter() {
        let out = to_csv("- a: 1\n  b: 2\n", "tab", true, "json", false, "key").unwrap();
        assert_eq!(out, "a\tb\n1\t2\n");
    }

    #[test]
    fn quote_all_wraps_every_field() {
        let out = to_csv("- a: 1\n  b: hi\n", "comma", true, "json", true, "key").unwrap();
        assert_eq!(out, "\"a\",\"b\"\n\"1\",\"hi\"\n");
    }

    #[test]
    fn blank_key_column_omits_the_key() {
        let out = to_csv("alice:\n  age: 30\n", "comma", true, "json", false, "").unwrap();
        assert_eq!(out, "age\n30\n");
    }

    #[test]
    fn quotes_embedded_commas() {
        let out = conv("- note: \"hi, there\"\n");
        assert_eq!(out, "note\n\"hi, there\"\n");
    }

    #[test]
    fn null_and_bool_cells() {
        // empty value → null → empty cell; true stays unquoted text.
        assert_eq!(conv("- a: null\n  b: true\n"), "a,b\n,true\n");
    }

    #[test]
    fn empty_input_errors() {
        assert!(to_csv("   ", "comma", true, "json", false, "key").is_err());
    }

    #[test]
    fn invalid_yaml_errors() {
        let e = to_csv("- a: [1, 2\n", "comma", true, "json", false, "key").unwrap_err();
        assert!(e.contains("YAML parse error"), "got: {e}");
    }

    #[test]
    fn unsupported_top_level_scalar_errors() {
        let e = to_csv("just a string", "comma", true, "json", false, "key").unwrap_err();
        assert!(e.contains("unsupported top-level YAML shape"), "got: {e}");
    }

    #[test]
    fn bad_delimiter_errors() {
        assert!(to_csv("- a: 1\n", "xx", true, "json", false, "key").is_err());
    }

    #[test]
    fn bad_array_mode_errors() {
        assert!(to_csv("- a: 1\n", "comma", true, "bogus", false, "key").is_err());
    }
}
