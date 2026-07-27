//! csv-to-ndjson core — pure compute, shared by the chat skill block and the web page.
//! Converts CSV text into NDJSON (newline-delimited JSON): one JSON value per line,
//! no enclosing array — the shape streaming ingest pipelines (jq, BigQuery load,
//! Elasticsearch bulk, log shippers) expect. No wafer/wasm-bindgen deps.

use serde_json::{Map, Value};

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

/// Convert CSV text to NDJSON. `headers`: treat the first row as field names, so
/// each row becomes a JSON object; when false, each row becomes a JSON array.
/// `parse_numbers`: coerce numeric-looking cells to JSON numbers. `parse_bools`:
/// coerce the literals `true`/`false`/`null` to JSON booleans/null. `trim`: strip
/// leading/trailing whitespace from every cell before conversion. When both
/// inference flags are off, every value is emitted as a JSON string (lossless).
pub fn to_ndjson(
    data: &str,
    delimiter: &str,
    headers: bool,
    parse_numbers: bool,
    parse_bools: bool,
    trim: bool,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let delim = delimiter_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(headers)
        .flexible(true)
        .from_reader(data.as_bytes());

    let mut lines: Vec<String> = Vec::new();

    if headers {
        let hdr = rdr
            .headers()
            .map_err(|e| format!("CSV parse error: {e}"))?
            .clone();
        for rec in rdr.records() {
            let rec = rec.map_err(|e| format!("CSV parse error: {e}"))?;
            let mut obj = Map::new();
            for (i, field) in rec.iter().enumerate() {
                let raw_key = hdr.get(i).unwrap_or("");
                let key = if trim { raw_key.trim() } else { raw_key }.to_string();
                obj.insert(key, cell_value(field, parse_numbers, parse_bools, trim));
            }
            let val = Value::Object(obj);
            lines.push(serde_json::to_string(&val).map_err(|e| format!("JSON encode error: {e}"))?);
        }
    } else {
        for rec in rdr.records() {
            let rec = rec.map_err(|e| format!("CSV parse error: {e}"))?;
            let row: Vec<Value> = rec
                .iter()
                .map(|f| cell_value(f, parse_numbers, parse_bools, trim))
                .collect();
            let val = Value::Array(row);
            lines.push(serde_json::to_string(&val).map_err(|e| format!("JSON encode error: {e}"))?);
        }
    }

    if lines.is_empty() {
        return Err("no data rows to convert".into());
    }
    // NDJSON = one JSON value per line, terminated by a trailing newline (RFC-ish
    // convention: consumers split on '\n'; the trailing newline is standard).
    let mut out = lines.join("\n");
    out.push('\n');
    Ok(out)
}

/// Turn one CSV cell into a JSON value, honoring the opt-in inference flags. With
/// both flags off every non-inferred cell stays a JSON string (empty cell → "").
fn cell_value(field: &str, parse_numbers: bool, parse_bools: bool, trim: bool) -> Value {
    let s = if trim { field.trim() } else { field };
    if parse_bools {
        match s {
            "true" => return Value::Bool(true),
            "false" => return Value::Bool(false),
            "null" => return Value::Null,
            "" => return Value::Null,
            _ => {}
        }
    }
    if parse_numbers && !s.is_empty() {
        if let Ok(i) = s.parse::<i64>() {
            // Reject re-serialization mismatches (e.g. "007", "+1") so ids/zips
            // that only look numeric stay strings.
            if i.to_string() == s {
                return Value::from(i);
            }
        }
        if let Ok(f) = s.parse::<f64>() {
            if f.is_finite() && Value::from(f).to_string() == s {
                return Value::from(f);
            }
        }
    }
    Value::String(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_by_default() {
        let out = to_ndjson("name,age\nAda,36\nGrace,", ",", true, false, false, false).unwrap();
        assert_eq!(
            out,
            "{\"name\":\"Ada\",\"age\":\"36\"}\n{\"name\":\"Grace\",\"age\":\"\"}\n"
        );
    }

    #[test]
    fn type_inference_opt_in() {
        let out = to_ndjson(
            "name,age,active\nAda,36,true\nGrace,,false",
            ",",
            true,
            true,
            true,
            false,
        )
        .unwrap();
        assert_eq!(
            out,
            "{\"name\":\"Ada\",\"age\":36,\"active\":true}\n{\"name\":\"Grace\",\"age\":null,\"active\":false}\n"
        );
    }

    #[test]
    fn no_headers_emits_arrays() {
        let out = to_ndjson("a,b\nc,d", ",", false, false, false, false).unwrap();
        assert_eq!(out, "[\"a\",\"b\"]\n[\"c\",\"d\"]\n");
    }

    #[test]
    fn quoted_field_with_comma_and_newline() {
        let out = to_ndjson(
            "name,note\n\"Ada, L\",\"line1\nline2\"",
            ",",
            true,
            false,
            false,
            false,
        )
        .unwrap();
        assert_eq!(out, "{\"name\":\"Ada, L\",\"note\":\"line1\\nline2\"}\n");
    }

    #[test]
    fn tab_delimiter_word() {
        let out = to_ndjson("a\tb\n1\t2", "tab", true, true, false, false).unwrap();
        assert_eq!(out, "{\"a\":1,\"b\":2}\n");
    }

    #[test]
    fn trim_whitespace() {
        let out = to_ndjson("name, age\n Ada , 36 ", ",", true, true, false, true).unwrap();
        assert_eq!(out, "{\"name\":\"Ada\",\"age\":36}\n");
    }

    #[test]
    fn leading_zero_stays_string_when_parsing_numbers() {
        let out = to_ndjson("zip\n007", ",", true, true, false, false).unwrap();
        assert_eq!(out, "{\"zip\":\"007\"}\n");
    }

    #[test]
    fn empty_input_errors() {
        assert!(to_ndjson("   ", ",", true, false, false, false).is_err());
    }

    #[test]
    fn header_only_errors() {
        let err = to_ndjson("a,b,c", ",", true, false, false, false).unwrap_err();
        assert!(err.contains("no data rows"), "got: {err}");
    }

    #[test]
    fn bad_delimiter_errors() {
        let err = to_ndjson("a,b\n1,2", "xx", true, false, false, false).unwrap_err();
        assert!(err.contains("single character"), "got: {err}");
    }
}
