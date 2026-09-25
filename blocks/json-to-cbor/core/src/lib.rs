//! json-to-cbor core — encode JSON values into RFC 8949 CBOR bytes.

use base64::Engine;
use serde_json::{Map, Number, Value};

#[derive(Clone, Debug)]
pub struct Options {
    pub output: String,
    pub canonical: bool,
    pub group: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            output: "hex".to_string(),
            canonical: true,
            group: 0,
        }
    }
}

pub fn run(json: &str) -> Result<String, String> {
    run_with_options(json, &Options::default())
}

pub fn run_with_options(json: &str, options: &Options) -> Result<String, String> {
    let input = json.trim();
    if input.is_empty() {
        return Err("json input is required".to_string());
    }
    if input.len() > 1_000_000 {
        return Err("json input is too large (max 1,000,000 bytes)".to_string());
    }
    let value: Value = serde_json::from_str(input).map_err(|e| format!("invalid JSON: {e}"))?;
    let mut bytes = Vec::new();
    encode_value(&value, &mut bytes, options.canonical)?;
    match options.output.trim().to_ascii_lowercase().as_str() {
        "hex" | "" => Ok(format_hex(&bytes, options.group)),
        "base64" => Ok(base64::engine::general_purpose::STANDARD.encode(&bytes)),
        "summary" => Ok(format_summary(input, &bytes, options.group)),
        "json" => Ok(format_json(input, &bytes, options.group)),
        other => Err(format!(
            "unknown output format '{other}' (use hex, base64, summary, or json)"
        )),
    }
}

fn encode_value(value: &Value, out: &mut Vec<u8>, canonical: bool) -> Result<(), String> {
    match value {
        Value::Null => out.push(0xf6),
        Value::Bool(false) => out.push(0xf4),
        Value::Bool(true) => out.push(0xf5),
        Value::Number(n) => encode_number(n, out)?,
        Value::String(s) => encode_text(s, out),
        Value::Array(items) => {
            encode_type_len(4, items.len() as u64, out);
            for item in items {
                encode_value(item, out, canonical)?;
            }
        }
        Value::Object(map) => encode_map(map, out, canonical)?,
    }
    Ok(())
}

fn encode_number(n: &Number, out: &mut Vec<u8>) -> Result<(), String> {
    if let Some(u) = n.as_u64() {
        encode_type_len(0, u, out);
        return Ok(());
    }
    if let Some(i) = n.as_i64() {
        if i >= 0 {
            encode_type_len(0, i as u64, out);
        } else {
            encode_type_len(1, (-1 - i) as u64, out);
        }
        return Ok(());
    }
    let f = n
        .as_f64()
        .ok_or_else(|| "JSON number cannot be represented as f64".to_string())?;
    out.push(0xfb);
    out.extend_from_slice(&f.to_bits().to_be_bytes());
    Ok(())
}

fn encode_text(s: &str, out: &mut Vec<u8>) {
    encode_type_len(3, s.len() as u64, out);
    out.extend_from_slice(s.as_bytes());
}

fn encode_map(map: &Map<String, Value>, out: &mut Vec<u8>, canonical: bool) -> Result<(), String> {
    encode_type_len(5, map.len() as u64, out);
    if canonical {
        let mut entries: Vec<(Vec<u8>, &Value)> = map
            .iter()
            .map(|(k, v)| {
                let mut key = Vec::new();
                encode_text(k, &mut key);
                (key, v)
            })
            .collect();
        entries.sort_by(|(ak, _), (bk, _)| ak.len().cmp(&bk.len()).then_with(|| ak.cmp(bk)));
        for (key, value) in entries {
            out.extend_from_slice(&key);
            encode_value(value, out, canonical)?;
        }
    } else {
        for (key, value) in map {
            encode_text(key, out);
            encode_value(value, out, canonical)?;
        }
    }
    Ok(())
}

fn encode_type_len(major: u8, len: u64, out: &mut Vec<u8>) {
    let head = major << 5;
    match len {
        0..=23 => out.push(head | len as u8),
        24..=0xff => out.extend_from_slice(&[head | 24, len as u8]),
        0x100..=0xffff => {
            out.push(head | 25);
            out.extend_from_slice(&(len as u16).to_be_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            out.push(head | 26);
            out.extend_from_slice(&(len as u32).to_be_bytes());
        }
        _ => {
            out.push(head | 27);
            out.extend_from_slice(&len.to_be_bytes());
        }
    }
}

fn format_hex(bytes: &[u8], group: u32) -> String {
    let raw = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    if group == 0 {
        return raw;
    }
    let chars_per_group = (group as usize).saturating_mul(2).max(2);
    raw.as_bytes()
        .chunks(chars_per_group)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_summary(input: &str, bytes: &[u8], group: u32) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    format!(
        "CBOR bytes: {}\nJSON bytes: {}\nCBOR hex: {}\nCBOR base64: {}",
        bytes.len(),
        input.as_bytes().len(),
        format_hex(bytes, group),
        b64
    )
}

fn format_json(input: &str, bytes: &[u8], group: u32) -> String {
    format!(
        "{{\"encoding\":\"cbor\",\"bytes\":{},\"json_bytes\":{},\"hex\":\"{}\",\"base64\":\"{}\"}}",
        bytes.len(),
        input.as_bytes().len(),
        format_hex(bytes, group),
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_simple_object_as_canonical_hex() {
        let out = run(r#"{"b":2,"a":1}"#).unwrap();
        assert_eq!(out, "a2616101616202");
    }

    #[test]
    fn supports_base64_and_arrays() {
        let opts = Options {
            output: "base64".into(),
            ..Options::default()
        };
        let out = run_with_options(r#"[1,true,null,"x"]"#, &opts).unwrap();
        assert_eq!(out, "hAH19mF4");
    }

    #[test]
    fn supports_grouped_summary() {
        let opts = Options {
            output: "summary".into(),
            group: 2,
            ..Options::default()
        };
        let out = run_with_options(r#"{"ok":true}"#, &opts).unwrap();
        assert!(out.contains("CBOR bytes:"));
        assert!(out.contains("a162 6f6b f5"));
    }

    #[test]
    fn rejects_bad_json() {
        assert!(run("{bad").unwrap_err().contains("invalid JSON"));
    }
}
