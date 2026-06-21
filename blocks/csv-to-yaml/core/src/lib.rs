//! gizza-ai/csv-to-yaml core — convert a CSV table into a YAML list of objects
//! keyed by the header row. Pure-Rust (`csv` + `serde_yml`). No wafer/wasm-bindgen
//! deps. Column order is preserved (serde_yml `Mapping` is insertion-ordered).

use serde_yml::value::Value as Yaml;
use serde_yml::Mapping;

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

/// Infer a YAML scalar from a cell: empty → null, true/false → bool, integer or
/// float → number, else string. When `infer` is false everything stays a string.
fn cell_value(s: &str, infer: bool) -> Yaml {
    if !infer {
        return Yaml::String(s.to_string());
    }
    if s.is_empty() {
        return Yaml::Null;
    }
    match s {
        "true" => return Yaml::Bool(true),
        "false" => return Yaml::Bool(false),
        _ => {}
    }
    if let Ok(i) = s.parse::<i64>() {
        // Avoid turning "007" or "+1" into a number (loses the original text).
        if i.to_string() == s {
            return Yaml::Number(i.into());
        }
    }
    if let Ok(f) = s.parse::<f64>() {
        if f.is_finite() && s.chars().any(|c| c == '.' || c == 'e' || c == 'E') {
            return Yaml::Number(f.into());
        }
    }
    Yaml::String(s.to_string())
}

/// Convert `data` (CSV with a header) into a YAML sequence of mappings.
pub fn to_yaml(data: &str, has_header: bool, infer: bool, delimiter: &str) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> =
        rdr.records().collect::<Result<_, _>>().map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() {
        return Err("no rows found".into());
    }
    let width = records.iter().map(|r| r.len()).max().unwrap_or(0);

    let (keys, body): (Vec<String>, &[csv::StringRecord]) = if has_header {
        let h = &records[0];
        let mut keys: Vec<String> = (0..width).map(|i| h.get(i).unwrap_or("").to_string()).collect();
        for (i, k) in keys.iter_mut().enumerate() {
            if k.is_empty() {
                *k = format!("col{}", i + 1);
            }
        }
        (keys, &records[1..])
    } else {
        ((1..=width).map(|i| format!("col{i}")).collect(), &records[..])
    };

    let mut seq: Vec<Yaml> = Vec::with_capacity(body.len());
    for rec in body {
        let mut map = Mapping::new();
        for (i, key) in keys.iter().enumerate() {
            let v = cell_value(rec.get(i).unwrap_or(""), infer);
            map.insert(Yaml::String(key.clone()), v);
        }
        seq.push(Yaml::Mapping(map));
    }

    serde_yml::to_string(&Yaml::Sequence(seq)).map_err(|e| format!("YAML serialize error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_of_objects() {
        let out = to_yaml("name,age\nAda,36\nBo,40", true, true, ",").unwrap();
        // serde_yml renders a sequence with "- name: Ada" style.
        assert!(out.contains("name: Ada"));
        assert!(out.contains("age: 36"));
        assert!(out.contains("name: Bo"));
        // number inference: 36 is unquoted
        assert!(!out.contains("age: '36'"));
    }

    #[test]
    fn type_inference() {
        let out = to_yaml("a,b,c\n1,true,", true, true, ",").unwrap();
        assert!(out.contains("a: 1"));
        assert!(out.contains("b: true"));
        assert!(out.contains("c: null"));
    }

    #[test]
    fn no_inference_keeps_strings() {
        let out = to_yaml("a\n1", true, false, ",").unwrap();
        // string "1" is quoted in YAML
        assert!(out.contains("a: '1'") || out.contains("a: \"1\""));
    }

    #[test]
    fn leading_zero_stays_string() {
        let out = to_yaml("code\n007", true, true, ",").unwrap();
        assert!(out.contains("'007'") || out.contains("\"007\""));
    }

    #[test]
    fn no_header_uses_col_keys() {
        let out = to_yaml("1,2\n3,4", false, true, ",").unwrap();
        assert!(out.contains("col1: 1"));
        assert!(out.contains("col2: 4"));
    }

    #[test]
    fn errors() {
        assert!(to_yaml("", true, true, ",").is_err());
        assert!(to_yaml("a\n1", true, true, "xx").is_err());
    }
}
