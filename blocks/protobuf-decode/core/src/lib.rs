//! protobuf-decode core — decode raw Protocol Buffers wire bytes into a
//! field-number / wire-type tree, recursing into nested length-delimited
//! messages, without needing a `.proto` schema. No wafer/wasm-bindgen deps;
//! shared by the chat skill block and the web page.
//!
//! The protobuf binary wire format is self-describing only at the level of
//! field numbers + wire types — without a schema you cannot know whether a
//! varint was meant to be a signed int, an enum, or a bool, nor whether a
//! length-delimited blob is a nested message, a `string`, or a `bytes` field.
//! So for each field we emit the field number, the raw wire type, and every
//! plausible interpretation of the value, recursing into length-delimited
//! payloads that parse cleanly as a sub-message.

use serde_json::{json, Map, Value};

/// Output rendering: a structured JSON tree, or a compact text outline.
pub fn decode(input: &str, encoding: &str, format: &str) -> Result<String, String> {
    let bytes = parse_bytes(input, encoding)?;
    let fields = decode_message(&bytes, 0)
        .map_err(|e| format!("not valid protobuf wire data: {e}"))?;

    match format {
        "" | "json" => {
            let tree = Value::Array(fields.into_iter().map(field_to_json).collect());
            serde_json::to_string_pretty(&tree).map_err(|e| e.to_string())
        }
        "text" => Ok(render_text(&fields, 0)),
        other => Err(format!(
            "invalid format {other:?}: expected \"json\" or \"text\""
        )),
    }
}

/// Decode a base64 / hex / `auto`-detected string of wire bytes.
fn parse_bytes(input: &str, encoding: &str) -> Result<Vec<u8>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("input is empty".into());
    }
    match encoding {
        "hex" => decode_hex(trimmed),
        "base64" => decode_base64(trimmed),
        "" | "auto" => {
            // Hex if it's purely hex digits (+ optional separators); else base64.
            let compact: String = trimmed
                .chars()
                .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-')
                .collect();
            if !compact.is_empty()
                && compact.chars().all(|c| c.is_ascii_hexdigit())
                && compact.len() % 2 == 0
            {
                decode_hex(trimmed)
            } else {
                decode_base64(trimmed)
            }
        }
        other => Err(format!(
            "invalid encoding {other:?}: expected \"base64\", \"hex\", or \"auto\""
        )),
    }
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let compact: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-')
        .collect();
    if compact.len() % 2 != 0 {
        return Err("hex input has an odd number of digits".into());
    }
    let mut out = Vec::with_capacity(compact.len() / 2);
    let bytes = compact.as_bytes();
    for pair in bytes.chunks(2) {
        let hi = hex_val(pair[0])?;
        let lo = hex_val(pair[1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_val(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!("invalid hex digit {:?}", c as char)),
    }
}

/// Standard + URL-safe base64, padding optional.
fn decode_base64(s: &str) -> Result<Vec<u8>, String> {
    const INVALID: u8 = 255;
    let val = |c: u8| -> u8 {
        match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => INVALID,
        }
    };
    let mut buf = 0u32;
    let mut bits = 0u32;
    let mut out = Vec::new();
    for &c in s.as_bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        let v = val(c);
        if v == INVALID {
            return Err(format!("invalid base64 character {:?}", c as char));
        }
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}

// --- wire-format parsing ---

#[derive(Debug, Clone)]
struct Field {
    number: u64,
    #[allow(dead_code)]
    wire_type: u8,
    value: WireValue,
}

#[derive(Debug, Clone)]
enum WireValue {
    Varint(u64),
    Fixed64(u64),
    Fixed32(u32),
    LengthDelimited(Vec<u8>),
}

/// Parse a complete byte slice as a sequence of protobuf fields. `depth`
/// bounds recursion into nested messages.
fn decode_message(bytes: &[u8], depth: usize) -> Result<Vec<Field>, String> {
    if depth > 64 {
        return Err("nesting too deep".into());
    }
    let mut fields = Vec::new();
    let mut pos = 0usize;
    while pos < bytes.len() {
        let (tag, n) = read_varint(&bytes[pos..])?;
        pos += n;
        let wire_type = (tag & 0x7) as u8;
        let number = tag >> 3;
        if number == 0 {
            return Err("field number 0 is not allowed".into());
        }
        let value = match wire_type {
            0 => {
                let (v, n) = read_varint(&bytes[pos..])?;
                pos += n;
                WireValue::Varint(v)
            }
            1 => {
                if pos + 8 > bytes.len() {
                    return Err("truncated 64-bit field".into());
                }
                let v = u64::from_le_bytes(bytes[pos..pos + 8].try_into().unwrap());
                pos += 8;
                WireValue::Fixed64(v)
            }
            2 => {
                let (len, n) = read_varint(&bytes[pos..])?;
                pos += n;
                let len = len as usize;
                if pos + len > bytes.len() {
                    return Err("length-delimited field exceeds input".into());
                }
                let payload = bytes[pos..pos + len].to_vec();
                pos += len;
                WireValue::LengthDelimited(payload)
            }
            5 => {
                if pos + 4 > bytes.len() {
                    return Err("truncated 32-bit field".into());
                }
                let v = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap());
                pos += 4;
                WireValue::Fixed32(v)
            }
            3 | 4 => {
                return Err(format!(
                    "wire type {wire_type} (deprecated start/end group) is not supported"
                ));
            }
            other => return Err(format!("unknown wire type {other}")),
        };
        fields.push(Field {
            number,
            wire_type,
            value,
        });
    }
    Ok(fields)
}

/// Read a base-128 varint; returns (value, bytes_consumed).
fn read_varint(bytes: &[u8]) -> Result<(u64, usize), String> {
    let mut result: u64 = 0;
    let mut shift = 0u32;
    for (i, &b) in bytes.iter().enumerate() {
        if shift >= 64 {
            return Err("varint overflows 64 bits".into());
        }
        result |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            return Ok((result, i + 1));
        }
        shift += 7;
    }
    Err("truncated varint".into())
}

// --- JSON / text rendering ---

fn field_to_json(f: Field) -> Value {
    let mut obj = Map::new();
    obj.insert("field".into(), json!(f.number));
    let (wt_name, mut interp) = match &f.value {
        WireValue::Varint(v) => {
            let signed = *v as i64;
            let zigzag = zigzag_decode(*v);
            let mut m = Map::new();
            m.insert("uint".into(), json!(v));
            m.insert("int".into(), json!(signed));
            m.insert("sint_zigzag".into(), json!(zigzag));
            if *v == 0 || *v == 1 {
                m.insert("bool".into(), json!(*v == 1));
            }
            ("varint", Value::Object(m))
        }
        WireValue::Fixed64(v) => {
            let mut m = Map::new();
            m.insert("uint64".into(), json!(v));
            m.insert("int64".into(), json!(*v as i64));
            m.insert("double".into(), json!(f64::from_bits(*v)));
            ("fixed64", Value::Object(m))
        }
        WireValue::Fixed32(v) => {
            let mut m = Map::new();
            m.insert("uint32".into(), json!(v));
            m.insert("int32".into(), json!(*v as i32));
            m.insert("float".into(), json!(f32::from_bits(*v) as f64));
            ("fixed32", Value::Object(m))
        }
        WireValue::LengthDelimited(payload) => {
            let mut m = Map::new();
            // Try to parse as a nested message first.
            if !payload.is_empty() {
                if let Ok(sub) = decode_message(payload, 1) {
                    if !sub.is_empty() {
                        m.insert(
                            "message".into(),
                            Value::Array(sub.into_iter().map(field_to_json).collect()),
                        );
                    }
                }
            } else {
                m.insert("message".into(), Value::Array(vec![]));
            }
            if let Ok(s) = std::str::from_utf8(payload) {
                if s.chars()
                    .all(|c| !c.is_control() || c == '\n' || c == '\t' || c == '\r')
                {
                    m.insert("string".into(), json!(s));
                }
            }
            m.insert("bytes_len".into(), json!(payload.len()));
            m.insert("hex".into(), json!(to_hex(payload)));
            ("length_delimited", Value::Object(m))
        }
    };
    obj.insert("wire_type".into(), json!(wt_name));
    if let Value::Object(ref mut m) = interp {
        obj.append(m);
    }
    Value::Object(obj)
}

fn render_text(fields: &[Field], indent: usize) -> String {
    let mut out = String::new();
    let pad = "  ".repeat(indent);
    for f in fields {
        match &f.value {
            WireValue::Varint(v) => {
                out.push_str(&format!("{pad}{}: varint {} (int {})\n", f.number, v, *v as i64));
            }
            WireValue::Fixed64(v) => {
                out.push_str(&format!(
                    "{pad}{}: fixed64 0x{:016x} (double {})\n",
                    f.number,
                    v,
                    f64::from_bits(*v)
                ));
            }
            WireValue::Fixed32(v) => {
                out.push_str(&format!(
                    "{pad}{}: fixed32 0x{:08x} (float {})\n",
                    f.number,
                    v,
                    f32::from_bits(*v)
                ));
            }
            WireValue::LengthDelimited(payload) => {
                let nested = if payload.is_empty() {
                    None
                } else {
                    decode_message(payload, 1).ok().filter(|s| !s.is_empty())
                };
                if let Some(sub) = nested {
                    out.push_str(&format!("{pad}{}: message {{\n", f.number));
                    out.push_str(&render_text(&sub, indent + 1));
                    out.push_str(&format!("{pad}}}\n"));
                } else if let Ok(s) = std::str::from_utf8(payload) {
                    out.push_str(&format!("{pad}{}: string {:?}\n", f.number, s));
                } else {
                    out.push_str(&format!(
                        "{pad}{}: bytes ({} bytes) {}\n",
                        f.number,
                        payload.len(),
                        to_hex(payload)
                    ));
                }
            }
        }
    }
    out
}

fn zigzag_decode(v: u64) -> i64 {
    ((v >> 1) as i64) ^ -((v & 1) as i64)
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    // message { 1: 150 } => 08 96 01
    #[test]
    fn decode_varint_field_hex() {
        let out = decode("08 96 01", "hex", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["field"], json!(1));
        assert_eq!(v[0]["wire_type"], json!("varint"));
        assert_eq!(v[0]["uint"], json!(150));
    }

    // message { 2: "testing" } => 12 07 74 65 73 74 69 6e 67
    #[test]
    fn decode_string_field() {
        let out = decode("120774657374696e67", "hex", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["field"], json!(2));
        assert_eq!(v[0]["wire_type"], json!("length_delimited"));
        assert_eq!(v[0]["string"], json!("testing"));
    }

    // nested: 1a 03 08 96 01  => field 3, message { 1: 150 }
    #[test]
    fn decode_nested_message() {
        let out = decode("1a03089601", "hex", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["field"], json!(3));
        let inner = &v[0]["message"];
        assert_eq!(inner[0]["field"], json!(1));
        assert_eq!(inner[0]["uint"], json!(150));
    }

    #[test]
    fn decode_base64_input() {
        // base64 of 08 96 01
        let out = decode("CJYB", "base64", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["uint"], json!(150));
    }

    #[test]
    fn auto_detects_hex() {
        let out = decode("089601", "auto", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["uint"], json!(150));
    }

    #[test]
    fn text_format_outline() {
        let out = decode("1a03089601", "hex", "text").unwrap();
        assert!(out.contains("3: message {"), "got: {out}");
        assert!(out.contains("1: varint 150"), "got: {out}");
    }

    #[test]
    fn fixed32_and_fixed64() {
        // field 1 fixed32 (wire 5): 0d 00 00 80 41 => float 16.0
        // field 2 fixed64 (wire 1): 11 00 00 00 00 00 00 f0 3f => double 1.0
        let out = decode("0d0000804111000000000000f03f", "hex", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["wire_type"], json!("fixed32"));
        assert_eq!(v[0]["float"], json!(16.0));
        assert_eq!(v[1]["wire_type"], json!("fixed64"));
        assert_eq!(v[1]["double"], json!(1.0));
    }

    #[test]
    fn rejects_invalid_input() {
        let err = decode("", "hex", "json").unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
        // Truncated varint.
        let err = decode("08", "hex", "json").unwrap_err();
        assert!(err.contains("protobuf"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_format() {
        let err = decode("089601", "hex", "yaml").unwrap_err();
        assert!(err.contains("invalid format"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_encoding() {
        let err = decode("089601", "octal", "json").unwrap_err();
        assert!(err.contains("invalid encoding"), "got: {err}");
    }
}
