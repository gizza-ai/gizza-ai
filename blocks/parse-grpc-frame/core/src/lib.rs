//! parse-grpc-frame core — split a gRPC length-prefixed message stream into
//! frames and decode each embedded protobuf payload into a field tree. Pure
//! compute, shared by the chat skill block and the web page; no wafer /
//! wasm-bindgen deps.
//!
//! gRPC carries every message in an HTTP/2 DATA-frame body as a
//! "Length-Prefixed-Message": a **1-byte compressed flag** (0 = uncompressed,
//! 1 = compressed), a **4-byte big-endian message length**, then exactly that
//! many payload bytes. Several messages can be concatenated back-to-back in one
//! body / captured stream. This tool walks the stream frame by frame and, for
//! each uncompressed frame, decodes the protobuf wire bytes into a
//! field-number / wire-type tree by reusing the schemaless protobuf-decode
//! core — so you can read an undocumented gRPC payload without a `.proto`.

use serde_json::{json, Map, Value};

/// One gRPC length-prefixed message.
struct Frame {
    /// The raw 1-byte compressed flag (0 or 1 per the gRPC spec).
    compressed_flag: u8,
    /// The 4-byte big-endian declared message length.
    length: u32,
    /// The payload bytes (protobuf wire bytes when uncompressed).
    payload: Vec<u8>,
}

/// Parse the gRPC frame stream in `input` (a base64/hex/`auto` byte string) and
/// render the frame list — `json` (default, a structured tree) or `text`
/// (a compact indented outline).
pub fn parse(input: &str, encoding: &str, format: &str) -> Result<String, String> {
    let bytes = parse_bytes(input, encoding)?;
    let frames = split_frames(&bytes)?;
    match format {
        "" | "json" => render_json(&frames, bytes.len()),
        "text" => Ok(render_text(&frames, bytes.len())),
        other => Err(format!(
            "invalid format {other:?}: expected \"json\" or \"text\""
        )),
    }
}

/// Split a byte stream into consecutive gRPC length-prefixed frames.
fn split_frames(bytes: &[u8]) -> Result<Vec<Frame>, String> {
    if bytes.is_empty() {
        return Err("input is empty".into());
    }
    let mut frames = Vec::new();
    let mut pos = 0usize;
    while pos < bytes.len() {
        if pos + 5 > bytes.len() {
            return Err(format!(
                "truncated gRPC frame header at byte {pos}: a frame needs a 5-byte prefix \
                 (1-byte compressed flag + 4-byte length) but only {} byte(s) remain — is the \
                 input a complete gRPC length-prefixed stream?",
                bytes.len() - pos
            ));
        }
        let flag = bytes[pos];
        if flag > 1 {
            return Err(format!(
                "frame at byte {pos} has compressed flag {flag}; gRPC permits only 0 \
                 (uncompressed) or 1 (compressed). This usually means the bytes are not a gRPC \
                 length-prefixed stream (e.g. it's a bare protobuf message — try the protobuf \
                 decoder instead)."
            ));
        }
        let length = u32::from_be_bytes(bytes[pos + 1..pos + 5].try_into().unwrap());
        let payload_start = pos + 5;
        let end = payload_start
            .checked_add(length as usize)
            .ok_or("frame length overflows")?;
        if end > bytes.len() {
            return Err(format!(
                "frame at byte {pos} declares a {length}-byte message but only {} payload byte(s) \
                 remain after its header — the stream is truncated",
                bytes.len() - payload_start
            ));
        }
        frames.push(Frame {
            compressed_flag: flag,
            length,
            payload: bytes[payload_start..end].to_vec(),
        });
        pos = end;
    }
    Ok(frames)
}

// --- JSON rendering ---

fn render_json(frames: &[Frame], total: usize) -> Result<String, String> {
    let arr: Vec<Value> = frames.iter().enumerate().map(|(i, f)| frame_to_json(i, f)).collect();
    let root = json!({
        "frame_count": frames.len(),
        "total_bytes": total,
        "frames": arr,
    });
    serde_json::to_string_pretty(&root).map_err(|e| e.to_string())
}

fn frame_to_json(index: usize, f: &Frame) -> Value {
    let mut obj = Map::new();
    obj.insert("index".into(), json!(index));
    obj.insert("compressed".into(), json!(f.compressed_flag == 1));
    obj.insert("compression_flag".into(), json!(f.compressed_flag));
    obj.insert("length".into(), json!(f.length));
    if f.compressed_flag == 1 {
        obj.insert(
            "note".into(),
            json!(
                "payload is marked compressed (flag = 1); decompress it (gzip/deflate per the \
                 grpc-encoding header) before decoding the protobuf"
            ),
        );
        obj.insert("payload_hex".into(), json!(to_hex(&f.payload)));
    } else {
        match decode_payload_json(&f.payload) {
            Ok(tree) => {
                obj.insert("message".into(), tree);
            }
            Err(e) => {
                obj.insert("decode_error".into(), json!(e));
                obj.insert("payload_hex".into(), json!(to_hex(&f.payload)));
            }
        }
    }
    Value::Object(obj)
}

/// Decode an uncompressed frame payload as protobuf wire bytes, reusing the
/// schemaless protobuf-decode core. An empty payload is a valid empty message.
fn decode_payload_json(payload: &[u8]) -> Result<Value, String> {
    if payload.is_empty() {
        return Ok(json!([]));
    }
    let hex = to_hex(payload);
    let s = gizza_ai_protobuf_decode_core::decode(&hex, "hex", "json")?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

// --- text rendering ---

fn render_text(frames: &[Frame], total: usize) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{} gRPC frame(s), {} byte(s) total\n",
        frames.len(),
        total
    ));
    for (i, f) in frames.iter().enumerate() {
        let comp = if f.compressed_flag == 1 {
            "compressed"
        } else {
            "uncompressed"
        };
        out.push_str(&format!(
            "\nframe {}: flag={} ({}), length={}\n",
            i, f.compressed_flag, comp, f.length
        ));
        if f.compressed_flag == 1 {
            out.push_str(&format!(
                "  (compressed payload — decompress before decoding) {}\n",
                to_hex(&f.payload)
            ));
            continue;
        }
        if f.payload.is_empty() {
            out.push_str("  (empty message)\n");
            continue;
        }
        match gizza_ai_protobuf_decode_core::decode(&to_hex(&f.payload), "hex", "text") {
            Ok(body) => {
                for line in body.lines() {
                    out.push_str(&format!("  {line}\n"));
                }
            }
            Err(e) => {
                out.push_str(&format!(
                    "  (payload is not valid protobuf: {e}) {}\n",
                    to_hex(&f.payload)
                ));
            }
        }
    }
    out
}

// --- byte parsing (base64 / hex / auto) ---

fn parse_bytes(input: &str, encoding: &str) -> Result<Vec<u8>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("input is empty".into());
    }
    match encoding {
        "hex" => decode_hex(trimmed),
        "base64" => decode_base64(trimmed),
        "" | "auto" => {
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
    for pair in compact.as_bytes().chunks(2) {
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

    // One frame wrapping message { 1: 150 } (08 96 01):
    //   flag 00 | length 00 00 00 03 | payload 08 96 01
    #[test]
    fn single_frame_json() {
        let out = parse("00 00 00 00 03 08 96 01", "hex", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["frame_count"], json!(1));
        assert_eq!(v["total_bytes"], json!(8));
        let fr = &v["frames"][0];
        assert_eq!(fr["index"], json!(0));
        assert_eq!(fr["compressed"], json!(false));
        assert_eq!(fr["length"], json!(3));
        assert_eq!(fr["message"][0]["field"], json!(1));
        assert_eq!(fr["message"][0]["uint"], json!(150));
    }

    // Two concatenated frames: {1:150} then {2:42}.
    #[test]
    fn multiple_frames() {
        let out = parse("0000000003089601 0000000002102a", "hex", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["frame_count"], json!(2));
        assert_eq!(v["frames"][0]["message"][0]["uint"], json!(150));
        assert_eq!(v["frames"][1]["index"], json!(1));
        assert_eq!(v["frames"][1]["message"][0]["field"], json!(2));
        assert_eq!(v["frames"][1]["message"][0]["uint"], json!(42));
    }

    // A compressed frame (flag = 1) is reported but not decoded.
    #[test]
    fn compressed_frame_not_decoded() {
        let out = parse("01 00 00 00 02 ab cd", "hex", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let fr = &v["frames"][0];
        assert_eq!(fr["compressed"], json!(true));
        assert_eq!(fr["compression_flag"], json!(1));
        assert_eq!(fr["payload_hex"], json!("abcd"));
        assert!(fr.get("message").is_none());
        assert!(fr["note"].as_str().unwrap().contains("compressed"));
    }

    // An empty (zero-length) message is valid framing.
    #[test]
    fn empty_message_frame() {
        let out = parse("0000000000", "hex", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["frame_count"], json!(1));
        assert_eq!(v["frames"][0]["length"], json!(0));
        assert_eq!(v["frames"][0]["message"], json!([]));
    }

    // A frame whose payload is not valid protobuf reports a decode_error but
    // still surfaces the bytes — the framing itself parsed fine.
    #[test]
    fn bad_payload_reports_decode_error() {
        // length 1, payload 08 = a truncated varint field (field 1, no value).
        let out = parse("0000000001 08", "hex", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let fr = &v["frames"][0];
        assert!(fr.get("decode_error").is_some());
        assert_eq!(fr["payload_hex"], json!("08"));
    }

    #[test]
    fn text_format_outline() {
        let out = parse("0000000003089601", "hex", "text").unwrap();
        assert!(out.contains("1 gRPC frame(s)"), "got: {out}");
        assert!(out.contains("frame 0: flag=0 (uncompressed), length=3"), "got: {out}");
        assert!(out.contains("1: varint 150"), "got: {out}");
    }

    #[test]
    fn base64_input() {
        // base64 of 00 00 00 00 03 08 96 01
        let out = parse("AAAAAAMIlgE=", "base64", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["frames"][0]["message"][0]["uint"], json!(150));
    }

    #[test]
    fn auto_detects_hex() {
        let out = parse("0000000003089601", "auto", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["frame_count"], json!(1));
    }

    #[test]
    fn rejects_empty() {
        let err = parse("", "hex", "json").unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn rejects_truncated_header() {
        // Only 3 bytes — can't form the 5-byte prefix.
        let err = parse("000000", "hex", "json").unwrap_err();
        assert!(err.contains("truncated gRPC frame header"), "got: {err}");
    }

    #[test]
    fn rejects_truncated_payload() {
        // Header says 10 bytes but only 2 follow.
        let err = parse("000000000a 0102", "hex", "json").unwrap_err();
        assert!(err.contains("truncated"), "got: {err}");
    }

    #[test]
    fn rejects_bad_compression_flag() {
        // flag 0x07 is not 0 or 1.
        let err = parse("0700000000", "hex", "json").unwrap_err();
        assert!(err.contains("compressed flag 7"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_format() {
        let err = parse("0000000003089601", "hex", "yaml").unwrap_err();
        assert!(err.contains("invalid format"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_encoding() {
        let err = parse("0000000003089601", "octal", "json").unwrap_err();
        assert!(err.contains("invalid encoding"), "got: {err}");
    }
}
