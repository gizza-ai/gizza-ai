//! gizza-ai/avro-to-json core — decode an Apache Avro Object Container File (OCF)
//! into readable JSON records. The OCF embeds its writer schema, so no external
//! `.avsc` is needed. Pure Rust (`apache-avro`); no wafer/wasm-bindgen deps.

use apache_avro::types::Value as AvroValue;
use apache_avro::Reader;
use serde_json::{Map, Value as Json};

/// Decode raw Avro bytes (interpreting `encoding`) into JSON.
///
/// `encoding` selects how the *string* `input` is turned into bytes:
/// `"auto"` (default) detects hex (only hex digits, even length) else base64;
/// `"base64"` / `"hex"` force one.
///
/// `format` selects the output shape:
/// - `"records"` (default) → a JSON array of the decoded records,
/// - `"ndjson"` → newline-delimited JSON (one record per line),
/// - `"full"` → `{ "schema": <writer schema>, "count": N, "records": [...] }`.
pub fn decode(input: &str, encoding: &str, format: &str) -> Result<String, String> {
    let bytes = decode_bytes(input, encoding)?;
    decode_ocf(&bytes, format)
}

/// Decode already-resolved OCF `bytes` into JSON (used by the file-input surfaces).
pub fn decode_ocf(bytes: &[u8], format: &str) -> Result<String, String> {
    if bytes.is_empty() {
        return Err("no Avro data: input is empty".into());
    }
    if bytes.len() < 4 || &bytes[..4] != b"Obj\x01" {
        return Err(
            "not an Avro Object Container File (missing the 'Obj\\x01' magic). This tool reads .avro OCF files, which embed their schema."
                .into(),
        );
    }
    let reader = Reader::new(bytes).map_err(|e| format!("failed to read Avro container: {e}"))?;
    // Capture the embedded writer schema (JSON) before consuming records.
    let schema_json: Json = serde_json::to_value(reader.writer_schema())
        .unwrap_or_else(|_| Json::String(reader.writer_schema().canonical_form()));

    let mut records: Vec<Json> = Vec::new();
    for (i, rec) in reader.enumerate() {
        let value = rec.map_err(|e| format!("error decoding record {i}: {e}"))?;
        records.push(avro_to_json(value));
    }

    let mode = if format.trim().is_empty() { "records" } else { format.trim() };
    match mode {
        "records" => serde_json::to_string_pretty(&records)
            .map_err(|e| format!("serialize records: {e}")),
        "ndjson" => {
            let lines: Result<Vec<String>, String> = records
                .iter()
                .map(|r| serde_json::to_string(r).map_err(|e| format!("serialize record: {e}")))
                .collect();
            Ok(lines?.join("\n"))
        }
        "full" => {
            let mut out = Map::new();
            out.insert("schema".into(), schema_json);
            out.insert("count".into(), Json::from(records.len()));
            out.insert("records".into(), Json::Array(records));
            serde_json::to_string_pretty(&Json::Object(out))
                .map_err(|e| format!("serialize output: {e}"))
        }
        other => Err(format!("unknown format '{other}' (use records, ndjson, or full)")),
    }
}

/// Turn the string `input` into bytes per `encoding`.
fn decode_bytes(input: &str, encoding: &str) -> Result<Vec<u8>, String> {
    let enc = if encoding.trim().is_empty() { "auto" } else { encoding.trim() };
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        return Err("input is empty".into());
    }
    let resolved = match enc {
        "auto" => {
            let no_sep: String =
                cleaned.chars().filter(|c| *c != ':' && *c != '-').collect();
            if !no_sep.is_empty()
                && no_sep.len() % 2 == 0
                && no_sep.chars().all(|c| c.is_ascii_hexdigit())
            {
                "hex"
            } else {
                "base64"
            }
        }
        e => e,
    };
    match resolved {
        "hex" => {
            let h: String = cleaned.chars().filter(|c| *c != ':' && *c != '-').collect();
            if h.len() % 2 != 0 {
                return Err("hex input has an odd number of digits".into());
            }
            (0..h.len())
                .step_by(2)
                .map(|i| {
                    u8::from_str_radix(&h[i..i + 2], 16)
                        .map_err(|_| format!("invalid hex byte '{}'", &h[i..i + 2]))
                })
                .collect()
        }
        "base64" => base64_decode(&cleaned),
        other => Err(format!("unknown encoding '{other}' (use auto, base64, or hex)")),
    }
}

/// Minimal base64 decoder (standard + URL-safe alphabets, optional padding).
fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' | b'-' => Some(62),
            b'/' | b'_' => Some(63),
            _ => None,
        }
    }
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0u8;
    for &c in s.as_bytes() {
        if c == b'=' {
            break;
        }
        let v = val(c).ok_or_else(|| format!("invalid base64 character '{}'", c as char))?;
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}

/// Convert an Avro `Value` into a `serde_json::Value` (the natural JSON shape).
fn avro_to_json(v: AvroValue) -> Json {
    match v {
        AvroValue::Null => Json::Null,
        AvroValue::Boolean(b) => Json::Bool(b),
        AvroValue::Int(i) | AvroValue::Date(i) | AvroValue::TimeMillis(i) => Json::from(i),
        AvroValue::Long(l)
        | AvroValue::TimeMicros(l)
        | AvroValue::TimestampMillis(l)
        | AvroValue::TimestampMicros(l)
        | AvroValue::LocalTimestampMillis(l)
        | AvroValue::LocalTimestampMicros(l)
        | AvroValue::TimestampNanos(l)
        | AvroValue::LocalTimestampNanos(l) => Json::from(l),
        AvroValue::Float(f) => json_num(f as f64),
        AvroValue::Double(d) => json_num(d),
        AvroValue::Bytes(b) | AvroValue::Fixed(_, b) => Json::String(base64_encode(&b)),
        AvroValue::String(s) | AvroValue::Enum(_, s) => Json::String(s),
        AvroValue::Union(_, inner) => avro_to_json(*inner),
        AvroValue::Array(items) => Json::Array(items.into_iter().map(avro_to_json).collect()),
        AvroValue::Map(m) => {
            let mut obj = Map::new();
            for (k, val) in m {
                obj.insert(k, avro_to_json(val));
            }
            Json::Object(obj)
        }
        AvroValue::Record(fields) => {
            let mut obj = Map::new();
            for (k, val) in fields {
                obj.insert(k, avro_to_json(val));
            }
            Json::Object(obj)
        }
        AvroValue::Decimal(d) => {
            // Represent a decimal by its big-endian two's-complement bytes (base64).
            let bytes: Vec<u8> = (&d).try_into().unwrap_or_default();
            Json::String(base64_encode(&bytes))
        }
        AvroValue::BigDecimal(bd) => Json::String(bd.to_string()),
        AvroValue::Duration(d) => {
            let months: u32 = d.months().into();
            let days: u32 = d.days().into();
            let millis: u32 = d.millis().into();
            let mut obj = Map::new();
            obj.insert("months".into(), Json::from(months));
            obj.insert("days".into(), Json::from(days));
            obj.insert("milliseconds".into(), Json::from(millis));
            Json::Object(obj)
        }
        AvroValue::Uuid(u) => Json::String(u.to_string()),
    }
}

fn json_num(f: f64) -> Json {
    serde_json::Number::from_f64(f).map(Json::Number).unwrap_or(Json::Null)
}

/// Standard base64 encode (with padding) — for bytes/fixed/decimal outputs.
fn base64_encode(data: &[u8]) -> String {
    const TBL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TBL[((n >> 18) & 63) as usize] as char);
        out.push(TBL[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { TBL[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { TBL[(n & 63) as usize] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use apache_avro::{Schema, Writer};
    use serde_json::json;

    fn sample_ocf() -> Vec<u8> {
        let schema = Schema::parse_str(
            r#"{
                "type": "record",
                "name": "User",
                "fields": [
                    {"name": "name", "type": "string"},
                    {"name": "age", "type": "int"},
                    {"name": "admin", "type": "boolean"}
                ]
            }"#,
        )
        .unwrap();
        let mut writer = Writer::new(&schema, Vec::new());
        for (n, a, adm) in [("Ada", 36, true), ("Linus", 54, false)] {
            let mut rec = apache_avro::types::Record::new(writer.schema()).unwrap();
            rec.put("name", n);
            rec.put("age", a);
            rec.put("admin", adm);
            writer.append(rec).unwrap();
        }
        writer.into_inner().unwrap()
    }

    #[test]
    fn decode_records() {
        let ocf = sample_ocf();
        let out = decode_ocf(&ocf, "records").unwrap();
        let parsed: Json = serde_json::from_str(&out).unwrap();
        assert_eq!(
            parsed,
            json!([
                {"name": "Ada", "age": 36, "admin": true},
                {"name": "Linus", "age": 54, "admin": false}
            ])
        );
    }

    #[test]
    fn decode_ndjson() {
        let ocf = sample_ocf();
        let out = decode_ocf(&ocf, "ndjson").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2);
        let first: Json = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["name"], json!("Ada"));
    }

    #[test]
    fn decode_full_has_schema_and_count() {
        let ocf = sample_ocf();
        let out = decode_ocf(&ocf, "full").unwrap();
        let parsed: Json = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["count"], json!(2));
        assert_eq!(parsed["records"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["schema"]["name"], json!("User"));
    }

    #[test]
    fn decode_from_base64_string() {
        let ocf = sample_ocf();
        let b64 = base64_encode(&ocf);
        let out = decode(&b64, "auto", "records").unwrap();
        let parsed: Json = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 2);
    }

    #[test]
    fn decode_from_hex_string() {
        let ocf = sample_ocf();
        let hex: String = ocf.iter().map(|b| format!("{b:02x}")).collect();
        let out = decode(&hex, "hex", "records").unwrap();
        let parsed: Json = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 2);
    }

    #[test]
    fn errors() {
        assert!(decode("", "auto", "records").is_err()); // empty
        assert!(decode_ocf(b"not avro", "records").is_err()); // bad magic
        assert!(decode_ocf(&sample_ocf(), "bogus").is_err()); // bad format
    }
}
