//! msgpack-to-json core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Decodes a MessagePack binary blob — given as a base64 or hex string — into
//! human-readable JSON. MessagePack has a few types JSON cannot hold natively
//! (raw binary `bin`, application `ext` types, and the `timestamp` extension),
//! so those are represented with a small, documented convention:
//!
//!   * `bin`  → a base64 (default) or hex STRING of the raw bytes.
//!   * `ext`  → `{ "$ext": <type>, "data": "<base64|hex>" }`.
//!   * timestamp ext (type -1) → an RFC 3339 / ISO 8601 UTC string.
//!   * map keys that are not strings → stringified (numbers/booleans/nil become
//!     their obvious text; containers become their compact JSON).
//!
//! Field/element order is preserved (serde_json `preserve_order`). If the input
//! holds several MessagePack values back-to-back (a stream), they are returned
//! as a JSON array in order. Invalid encoding or malformed MessagePack produces
//! a clear error instead of a panic.

use base64::Engine as _;
use rmpv::Value as RV;
use serde_json::{Map as JMap, Number, Value as J};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Decode `input` (base64/hex bytes of one or more MessagePack values) to JSON.
///
/// * `input_format`: `"auto"` (default — detect hex vs base64), `"hex"`, or
///   `"base64"`.
/// * `indent`: spaces per nesting level (0-8); `0` minifies to one line.
/// * `binary_format`: how raw `bin` values and `ext` payloads are encoded in
///   the JSON — `"base64"` (default) or `"hex"`.
pub fn run(
    input: &str,
    input_format: &str,
    indent: usize,
    binary_format: &str,
) -> Result<String, String> {
    let bytes = decode_input(input, input_format)?;
    if bytes.is_empty() {
        return Err("input decoded to zero bytes — nothing to decode".into());
    }

    let bin = BinFmt::parse(binary_format)?;

    // Decode every top-level MessagePack value (a stream may hold several).
    let mut values: Vec<rmpv::Value> = Vec::new();
    let mut cursor: &[u8] = &bytes;
    while !cursor.is_empty() {
        match rmpv::decode::read_value(&mut cursor) {
            Ok(v) => values.push(v),
            Err(e) => {
                let at = bytes.len() - cursor.len();
                return Err(format!(
                    "not valid MessagePack (byte offset {at}): {e}"
                ));
            }
        }
    }

    let json = if values.len() == 1 {
        to_json(values.into_iter().next().unwrap(), bin)
    } else {
        J::Array(values.into_iter().map(|v| to_json(v, bin)).collect())
    };

    if indent > 8 {
        return Err("indent must be between 0 and 8".into());
    }
    Ok(serialize(&json, indent))
}

// ---------------------------------------------------------------------------
// Input decoding (bytes from a base64 / hex string)
// ---------------------------------------------------------------------------

fn decode_input(input: &str, format: &str) -> Result<Vec<u8>, String> {
    match format.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => {
            if looks_like_hex(input) {
                decode_hex(input)
            } else {
                decode_base64(input)
            }
        }
        "hex" => decode_hex(input),
        "base64" | "b64" => decode_base64(input),
        other => Err(format!(
            "unknown input_format '{other}' — use 'auto', 'hex', or 'base64'"
        )),
    }
}

/// True when, after stripping whitespace and any `0x` markers, the string is a
/// non-empty even-length run of hex digits — the only shape `decode_hex` accepts.
fn looks_like_hex(input: &str) -> bool {
    let cleaned = strip_hex_noise(input);
    !cleaned.is_empty()
        && cleaned.len() % 2 == 0
        && cleaned.bytes().all(|b| b.is_ascii_hexdigit())
}

fn strip_hex_noise(input: &str) -> String {
    let lower = input.replace("0x", "").replace("0X", "");
    lower
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-' && *c != ',')
        .collect()
}

fn decode_hex(input: &str) -> Result<Vec<u8>, String> {
    let cleaned = strip_hex_noise(input);
    if cleaned.is_empty() {
        return Err("no hex digits found in the input".into());
    }
    hex::decode(&cleaned).map_err(|e| format!("invalid hex input: {e}"))
}

fn decode_base64(input: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        return Err("no base64 characters found in the input".into());
    }
    // Try standard and URL-safe alphabets, with padding optional.
    for engine in [
        &base64::engine::general_purpose::STANDARD,
        &base64::engine::general_purpose::STANDARD_NO_PAD,
        &base64::engine::general_purpose::URL_SAFE,
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
    ] {
        if let Ok(bytes) = engine.decode(cleaned.trim_end_matches('=')) {
            return Ok(bytes);
        }
        if let Ok(bytes) = engine.decode(&cleaned) {
            return Ok(bytes);
        }
    }
    Err("invalid base64 input (also not valid hex — set input_format if the encoding is ambiguous)".into())
}

// ---------------------------------------------------------------------------
// MessagePack value → JSON value
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum BinFmt {
    Base64,
    Hex,
}

impl BinFmt {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "base64" | "b64" => Ok(BinFmt::Base64),
            "hex" => Ok(BinFmt::Hex),
            other => Err(format!(
                "unknown binary_format '{other}' — use 'base64' or 'hex'"
            )),
        }
    }

    fn encode(self, bytes: &[u8]) -> String {
        match self {
            BinFmt::Base64 => base64::engine::general_purpose::STANDARD.encode(bytes),
            BinFmt::Hex => hex::encode(bytes),
        }
    }
}

fn to_json(v: RV, bin: BinFmt) -> J {
    match v {
        RV::Nil => J::Null,
        RV::Boolean(b) => J::Bool(b),
        RV::Integer(i) => {
            if let Some(u) = i.as_u64() {
                J::Number(Number::from(u))
            } else if let Some(n) = i.as_i64() {
                J::Number(Number::from(n))
            } else {
                // Unreachable for well-formed MessagePack integers.
                J::Null
            }
        }
        RV::F32(f) => finite_number(f as f64),
        RV::F64(f) => finite_number(f),
        RV::String(s) => match s.as_str() {
            Some(text) => J::String(text.to_string()),
            None => J::String(String::from_utf8_lossy(s.as_bytes()).into_owned()),
        },
        RV::Binary(bytes) => J::String(bin.encode(&bytes)),
        RV::Array(items) => J::Array(items.into_iter().map(|x| to_json(x, bin)).collect()),
        RV::Map(pairs) => {
            let mut obj = JMap::new();
            for (k, val) in pairs {
                obj.insert(key_to_string(k, bin), to_json(val, bin));
            }
            J::Object(obj)
        }
        RV::Ext(ty, data) => ext_to_json(ty, data, bin),
    }
}

/// JSON has no NaN/Infinity; represent non-finite floats as null.
fn finite_number(x: f64) -> J {
    Number::from_f64(x).map(J::Number).unwrap_or(J::Null)
}

fn key_to_string(k: RV, bin: BinFmt) -> String {
    match k {
        RV::String(s) => match s.as_str() {
            Some(text) => text.to_string(),
            None => String::from_utf8_lossy(s.as_bytes()).into_owned(),
        },
        RV::Nil => "null".to_string(),
        RV::Boolean(b) => b.to_string(),
        RV::Integer(i) => i.to_string(),
        RV::F32(f) => f.to_string(),
        RV::F64(f) => f.to_string(),
        RV::Binary(bytes) => bin.encode(&bytes),
        other => serialize(&to_json(other, bin), 0),
    }
}

/// MessagePack ext type -1 is the reserved `timestamp` extension.
fn ext_to_json(ty: i8, data: Vec<u8>, bin: BinFmt) -> J {
    if ty == -1 {
        if let Some(ts) = decode_timestamp(&data) {
            return J::String(ts);
        }
    }
    let mut obj = JMap::new();
    obj.insert("$ext".to_string(), J::Number(Number::from(ty as i64)));
    obj.insert("data".to_string(), J::String(bin.encode(&data)));
    J::Object(obj)
}

/// Decode a MessagePack timestamp extension payload (4, 8, or 12 bytes) into an
/// RFC 3339 UTC string. Returns None if the length isn't a valid timestamp form.
fn decode_timestamp(data: &[u8]) -> Option<String> {
    let (secs, nanos): (i64, u32) = match data.len() {
        4 => {
            let s = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            (s as i64, 0)
        }
        8 => {
            let v = u64::from_be_bytes([
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
            ]);
            let nanos = (v >> 34) as u32; // top 30 bits
            let secs = (v & 0x0003_ffff_ffff) as i64; // low 34 bits
            (secs, nanos)
        }
        12 => {
            let nanos = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            let secs = i64::from_be_bytes([
                data[4], data[5], data[6], data[7], data[8], data[9], data[10], data[11],
            ]);
            (secs, nanos)
        }
        _ => return None,
    };
    if nanos >= 1_000_000_000 {
        return None;
    }
    Some(format_rfc3339(secs, nanos))
}

/// Format a Unix timestamp (seconds since 1970-01-01, plus nanoseconds) as an
/// RFC 3339 UTC string. Hand-rolled civil-from-days (Howard Hinnant) so no
/// date crate is needed; handles pre-1970 (negative) seconds correctly.
fn format_rfc3339(secs: i64, nanos: u32) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days);

    let frac = if nanos == 0 {
        String::new()
    } else {
        // Trim trailing zeros from the 9-digit nanosecond field.
        let mut f = format!(".{nanos:09}");
        while f.ends_with('0') {
            f.pop();
        }
        f
    };
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}{frac}Z")
}

/// Convert a count of days since the Unix epoch to (year, month, day).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as i64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ---------------------------------------------------------------------------
// Serialization with a configurable indent
// ---------------------------------------------------------------------------

fn serialize(value: &J, indent: usize) -> String {
    if indent == 0 {
        return serde_json::to_string(value).unwrap_or_default();
    }
    let pad = " ".repeat(indent);
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(pad.as_bytes());
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    use serde::Serialize;
    if value.serialize(&mut ser).is_err() {
        return serde_json::to_string_pretty(value).unwrap_or_default();
    }
    String::from_utf8(buf).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // {"compact": true, "schema": 0} — the canonical MessagePack example.
    const COMPACT_HEX: &str = "82a7636f6d70616374c3a6736368656d6100";

    #[test]
    fn happy_hex_map() {
        let out = run(COMPACT_HEX, "auto", 2, "base64").unwrap();
        assert_eq!(out, "{\n  \"compact\": true,\n  \"schema\": 0\n}");
    }

    #[test]
    fn happy_base64_same_as_hex() {
        let bytes = hex::decode(COMPACT_HEX).unwrap();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let out = run(&b64, "base64", 0, "base64").unwrap();
        assert_eq!(out, "{\"compact\":true,\"schema\":0}");
    }

    #[test]
    fn preserves_key_order() {
        // map {"z":1,"a":2} — order must survive, not sort.
        let out = run("82a17a01a16102", "hex", 0, "base64").unwrap();
        assert_eq!(out, "{\"z\":1,\"a\":2}");
    }

    #[test]
    fn nested_array_and_types() {
        // [1, "two", nil, true, 3.5]
        let out = run("95 01 a374776f c0 c3 cb400c000000000000", "hex", 0, "base64").unwrap();
        assert_eq!(out, "[1,\"two\",null,true,3.5]");
    }

    #[test]
    fn uint64_preserves_full_precision() {
        // 0xcf ffffffffffffffff = 18446744073709551615 (u64::MAX)
        let out = run("cfffffffffffffffff", "hex", 0, "base64").unwrap();
        assert_eq!(out, "18446744073709551615");
    }

    #[test]
    fn negative_int() {
        // 0xd0 0x9c = int8 -100
        let out = run("d09c", "hex", 0, "base64").unwrap();
        assert_eq!(out, "-100");
    }

    #[test]
    fn binary_as_base64_then_hex() {
        // bin8 length 3: 0xc4 0x03 0x01 0x02 0x03
        let b64 = run("c403010203", "hex", 0, "base64").unwrap();
        assert_eq!(b64, "\"AQID\"");
        let hx = run("c403010203", "hex", 0, "hex").unwrap();
        assert_eq!(hx, "\"010203\"");
    }

    #[test]
    fn timestamp_ext_becomes_iso() {
        // fixext4 (0xd6), type -1 (0xff), 2018-01-01T00:00:00Z = 0x5a497a00
        let out = run("d6ff5a497a00", "hex", 0, "base64").unwrap();
        assert_eq!(out, "\"2018-01-01T00:00:00Z\"");
    }

    #[test]
    fn generic_ext_object() {
        // fixext1 (0xd4), type 42 (0x2a), data 0x07
        let out = run("d42a07", "hex", 0, "base64").unwrap();
        assert_eq!(out, "{\"$ext\":42,\"data\":\"Bw==\"}");
    }

    #[test]
    fn stream_of_two_values_becomes_array() {
        // 0x01 0x02 — two positive fixints back to back
        let out = run("0102", "hex", 0, "base64").unwrap();
        assert_eq!(out, "[1,2]");
    }

    #[test]
    fn indent_four_spaces() {
        let out = run("81a16101", "hex", 4, "base64").unwrap();
        assert_eq!(out, "{\n    \"a\": 1\n}");
    }

    #[test]
    fn err_invalid_hex() {
        let err = run("zzzz", "hex", 2, "base64").unwrap_err();
        assert!(err.contains("hex"), "got: {err}");
    }

    #[test]
    fn err_truncated_msgpack() {
        // 0x93 declares a 3-element array but supplies only one element.
        let err = run("9301", "hex", 2, "base64").unwrap_err();
        assert!(err.contains("MessagePack"), "got: {err}");
    }

    #[test]
    fn err_empty_input() {
        let err = run("", "auto", 2, "base64").unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn non_string_map_key_stringified() {
        // map { 1: "a" } — fixint key 0x01
        let out = run("81 01 a161", "hex", 0, "base64").unwrap();
        assert_eq!(out, "{\"1\":\"a\"}");
    }
}
