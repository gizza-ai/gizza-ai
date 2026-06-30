//! parse-websocket-frame core — decode a single WebSocket frame (RFC 6455) into
//! its header fields and unmasked payload. Pure compute, shared by the chat skill
//! block and the web page; no wafer / wasm-bindgen deps.
//!
//! A WebSocket frame begins with two bytes:
//!
//! ```text
//!  0                   1                   2                   3
//!  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
//! +-+-+-+-+-------+-+-------------+-------------------------------+
//! |F|R|R|R| opcode|M| Payload len |    Extended payload length    |
//! |I|S|S|S|  (4)  |A|     (7)     |             (16/64)           |
//! |N|V|V|V|       |S|             |   (if payload len==126/127)   |
//! | |1|2|3|       |K|             |                               |
//! +-+-+-+-+-------+-+-------------+ - - - - - - - - - - - - - - - +
//! ```
//!
//! byte0 → FIN (0x80), RSV1/2/3 (0x40/0x20/0x10), opcode (low nibble).
//! byte1 → MASK (0x80) + 7-bit base length; 126 → next 2 bytes (u16 BE) are the
//! length, 127 → next 8 bytes (u64 BE) are the length. If MASK is set a 4-byte
//! masking key follows, and the payload is XOR-unmasked with key[i % 4].

use serde_json::{json, Map, Value};

/// Decoded WebSocket frame.
struct Frame {
    fin: bool,
    rsv1: bool,
    rsv2: bool,
    rsv3: bool,
    opcode: u8,
    masked: bool,
    payload_length: u64,
    masking_key: Option<[u8; 4]>,
    /// The unmasked payload bytes.
    payload: Vec<u8>,
    /// Total bytes consumed by the frame (header + payload).
    frame_bytes: usize,
}

/// Human-readable opcode name per RFC 6455 §5.2 / §11.8.
fn opcode_name(op: u8) -> &'static str {
    match op {
        0x0 => "continuation",
        0x1 => "text",
        0x2 => "binary",
        0x3..=0x7 => "reserved (non-control)",
        0x8 => "close",
        0x9 => "ping",
        0xA => "pong",
        0xB..=0xF => "reserved (control)",
        _ => "reserved",
    }
}

/// Parse the WebSocket frame in `input` (a base64/hex/`auto` byte string) and
/// render it — `json` (default, a structured object) or `text` (a compact
/// human-readable summary).
pub fn parse(input: &str, encoding: &str, format: &str) -> Result<String, String> {
    let bytes = parse_bytes(input, encoding)?;
    let frame = decode_frame(&bytes)?;
    match format {
        "" | "json" => render_json(&frame, bytes.len()),
        "text" => Ok(render_text(&frame, bytes.len())),
        other => Err(format!(
            "invalid format {other:?}: expected \"json\" or \"text\""
        )),
    }
}

/// Decode the first WebSocket frame at the start of `bytes`.
fn decode_frame(bytes: &[u8]) -> Result<Frame, String> {
    if bytes.len() < 2 {
        return Err(format!(
            "truncated WebSocket frame: need at least 2 bytes for the base header (FIN/opcode + \
             MASK/length) but only {} byte(s) were given — is this a complete frame?",
            bytes.len()
        ));
    }

    let b0 = bytes[0];
    let b1 = bytes[1];

    let fin = b0 & 0x80 != 0;
    let rsv1 = b0 & 0x40 != 0;
    let rsv2 = b0 & 0x20 != 0;
    let rsv3 = b0 & 0x10 != 0;
    let opcode = b0 & 0x0F;

    let masked = b1 & 0x80 != 0;
    let base_len = (b1 & 0x7F) as u64;

    // Read the (possibly extended) payload length.
    let mut pos = 2usize;
    let payload_length = match base_len {
        126 => {
            if bytes.len() < pos + 2 {
                return Err(format!(
                    "truncated WebSocket frame: payload-length marker 126 needs a 2-byte extended \
                     length at byte {pos}, but only {} byte(s) remain",
                    bytes.len() - pos
                ));
            }
            let len = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as u64;
            pos += 2;
            len
        }
        127 => {
            if bytes.len() < pos + 8 {
                return Err(format!(
                    "truncated WebSocket frame: payload-length marker 127 needs an 8-byte extended \
                     length at byte {pos}, but only {} byte(s) remain",
                    bytes.len() - pos
                ));
            }
            let len = u64::from_be_bytes(bytes[pos..pos + 8].try_into().unwrap());
            pos += 8;
            len
        }
        n => n,
    };

    // The masking key, if present.
    let masking_key = if masked {
        if bytes.len() < pos + 4 {
            return Err(format!(
                "truncated WebSocket frame: the MASK bit is set so a 4-byte masking key is \
                 expected at byte {pos}, but only {} byte(s) remain",
                bytes.len() - pos
            ));
        }
        let key = [bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]];
        pos += 4;
        Some(key)
    } else {
        None
    };

    // The payload.
    let payload_len_usize = usize::try_from(payload_length)
        .map_err(|_| format!("payload length {payload_length} is too large to address"))?;
    let payload_end = pos
        .checked_add(payload_len_usize)
        .ok_or("payload length overflows the address space")?;
    if bytes.len() < payload_end {
        return Err(format!(
            "truncated WebSocket frame: the header declares a {payload_length}-byte payload but \
             only {} payload byte(s) follow the header — the frame is incomplete",
            bytes.len() - pos
        ));
    }

    // Unmask the payload (XOR with key[i % 4]) when masked; otherwise copy as-is.
    let raw = &bytes[pos..payload_end];
    let payload: Vec<u8> = match masking_key {
        Some(key) => raw
            .iter()
            .enumerate()
            .map(|(i, &b)| b ^ key[i % 4])
            .collect(),
        None => raw.to_vec(),
    };

    Ok(Frame {
        fin,
        rsv1,
        rsv2,
        rsv3,
        opcode,
        masked,
        payload_length,
        masking_key,
        payload,
        frame_bytes: payload_end,
    })
}

// --- JSON rendering ---

fn render_json(f: &Frame, total: usize) -> Result<String, String> {
    let mut obj = Map::new();
    obj.insert("fin".into(), json!(f.fin));
    obj.insert("rsv1".into(), json!(f.rsv1));
    obj.insert("rsv2".into(), json!(f.rsv2));
    obj.insert("rsv3".into(), json!(f.rsv3));
    obj.insert("opcode".into(), json!(f.opcode));
    obj.insert("opcode_name".into(), json!(opcode_name(f.opcode)));
    obj.insert("masked".into(), json!(f.masked));
    obj.insert("payload_length".into(), json!(f.payload_length));
    if let Some(key) = f.masking_key {
        obj.insert("masking_key".into(), json!(to_hex(&key)));
    } else {
        obj.insert("masking_key".into(), Value::Null);
    }
    obj.insert("payload_hex".into(), json!(to_hex(&f.payload)));
    // Decode payload as UTF-8 text for text frames (opcode 0x1).
    if f.opcode == 0x1 {
        if let Ok(s) = std::str::from_utf8(&f.payload) {
            obj.insert("payload_text".into(), json!(s));
        }
    }
    // For close frames, surface the status code + optional reason.
    if f.opcode == 0x8 {
        if let Some(close) = close_details(&f.payload) {
            obj.insert("close".into(), close);
        }
    }
    obj.insert("frame_bytes".into(), json!(f.frame_bytes));
    if f.frame_bytes < total {
        obj.insert(
            "note".into(),
            json!(format!(
                "{} trailing byte(s) after this frame were not decoded (only the first frame is \
                 parsed)",
                total - f.frame_bytes
            )),
        );
    }
    serde_json::to_string_pretty(&Value::Object(obj)).map_err(|e| e.to_string())
}

/// For a close frame, split out the 2-byte big-endian status code and the
/// optional UTF-8 reason (RFC 6455 §5.5.1).
fn close_details(payload: &[u8]) -> Option<Value> {
    if payload.len() < 2 {
        return None;
    }
    let code = u16::from_be_bytes([payload[0], payload[1]]);
    let mut obj = Map::new();
    obj.insert("code".into(), json!(code));
    if let Ok(reason) = std::str::from_utf8(&payload[2..]) {
        if !reason.is_empty() {
            obj.insert("reason".into(), json!(reason));
        }
    }
    Some(Value::Object(obj))
}

// --- text rendering ---

fn render_text(f: &Frame, total: usize) -> String {
    let mut out = String::new();
    out.push_str("WebSocket frame\n");
    out.push_str(&format!("  FIN:     {}\n", f.fin));
    out.push_str(&format!("  RSV1/2/3: {} {} {}\n", f.rsv1, f.rsv2, f.rsv3));
    out.push_str(&format!(
        "  opcode:  0x{:x} ({})\n",
        f.opcode,
        opcode_name(f.opcode)
    ));
    out.push_str(&format!("  masked:  {}\n", f.masked));
    if let Some(key) = f.masking_key {
        out.push_str(&format!("  mask key: {}\n", to_hex(&key)));
    }
    out.push_str(&format!("  payload length: {}\n", f.payload_length));
    out.push_str(&format!("  payload (hex): {}\n", to_hex(&f.payload)));
    if f.opcode == 0x1 {
        if let Ok(s) = std::str::from_utf8(&f.payload) {
            out.push_str(&format!("  payload (text): {s}\n"));
        }
    }
    if f.opcode == 0x8 && f.payload.len() >= 2 {
        let code = u16::from_be_bytes([f.payload[0], f.payload[1]]);
        out.push_str(&format!("  close code: {code}\n"));
        if let Ok(reason) = std::str::from_utf8(&f.payload[2..]) {
            if !reason.is_empty() {
                out.push_str(&format!("  close reason: {reason}\n"));
            }
        }
    }
    if f.frame_bytes < total {
        out.push_str(&format!(
            "  note: {} trailing byte(s) after this frame were not decoded\n",
            total - f.frame_bytes
        ));
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

    // Unmasked text frame "Hello": FIN=1, opcode=1, no mask, len=5.
    //   0x81 0x05 48 65 6c 6c 6f
    #[test]
    fn unmasked_text_frame() {
        let out = parse("81 05 48 65 6c 6c 6f", "hex", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["fin"], json!(true));
        assert_eq!(v["rsv1"], json!(false));
        assert_eq!(v["opcode"], json!(1));
        assert_eq!(v["opcode_name"], json!("text"));
        assert_eq!(v["masked"], json!(false));
        assert_eq!(v["payload_length"], json!(5));
        assert_eq!(v["masking_key"], Value::Null);
        assert_eq!(v["payload_hex"], json!("48656c6c6f"));
        assert_eq!(v["payload_text"], json!("Hello"));
    }

    // Masked client text frame "Hello" from RFC 6455 §5.7:
    //   0x81 0x85 37 fa 21 3d 7f 9f 4d 51 58  →  unmasks to "Hello".
    #[test]
    fn masked_client_frame() {
        let out = parse("81 85 37 fa 21 3d 7f 9f 4d 51 58", "hex", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["fin"], json!(true));
        assert_eq!(v["opcode"], json!(1));
        assert_eq!(v["masked"], json!(true));
        assert_eq!(v["masking_key"], json!("37fa213d"));
        assert_eq!(v["payload_length"], json!(5));
        assert_eq!(v["payload_text"], json!("Hello"));
        assert_eq!(v["payload_hex"], json!("48656c6c6f"));
    }

    // 126 extended length: a 256-byte unmasked binary frame.
    //   0x82 0x7e 0x0100 <256 bytes>
    #[test]
    fn extended_length_126() {
        let mut bytes = vec![0x82u8, 0x7e, 0x01, 0x00];
        bytes.extend(std::iter::repeat(0xAB).take(256));
        let hex = to_hex(&bytes);
        let out = parse(&hex, "hex", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["opcode"], json!(2));
        assert_eq!(v["opcode_name"], json!("binary"));
        assert_eq!(v["payload_length"], json!(256));
        assert_eq!(v["masked"], json!(false));
    }

    // 127 extended length: a 70000-byte unmasked binary frame.
    #[test]
    fn extended_length_127() {
        let len: u64 = 70000;
        let mut bytes = vec![0x82u8, 0x7f];
        bytes.extend_from_slice(&len.to_be_bytes());
        bytes.extend(std::iter::repeat(0x00).take(len as usize));
        let hex = to_hex(&bytes);
        let out = parse(&hex, "hex", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["payload_length"], json!(70000));
    }

    // Ping frame, no payload: 0x89 0x00.
    #[test]
    fn ping_frame() {
        let out = parse("89 00", "hex", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["opcode"], json!(9));
        assert_eq!(v["opcode_name"], json!("ping"));
        assert_eq!(v["payload_length"], json!(0));
    }

    // Pong frame: 0x8a 0x00.
    #[test]
    fn pong_frame() {
        let out = parse("8a 00", "hex", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["opcode"], json!(10));
        assert_eq!(v["opcode_name"], json!("pong"));
    }

    // Close frame with status 1000 ("normal") + reason "bye":
    //   0x88 0x05 03 e8 62 79 65
    #[test]
    fn close_frame() {
        let out = parse("88 05 03 e8 62 79 65", "hex", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["opcode"], json!(8));
        assert_eq!(v["opcode_name"], json!("close"));
        assert_eq!(v["close"]["code"], json!(1000));
        assert_eq!(v["close"]["reason"], json!("bye"));
    }

    // Continuation opcode 0x0, FIN clear.
    #[test]
    fn continuation_opcode() {
        let out = parse("00 00", "hex", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["opcode"], json!(0));
        assert_eq!(v["opcode_name"], json!("continuation"));
        assert_eq!(v["fin"], json!(false));
    }

    // RSV bits set: 0xF1 = FIN + RSV1 + RSV2 + RSV3 + opcode 1.
    #[test]
    fn rsv_bits() {
        let out = parse("f1 00", "hex", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["fin"], json!(true));
        assert_eq!(v["rsv1"], json!(true));
        assert_eq!(v["rsv2"], json!(true));
        assert_eq!(v["rsv3"], json!(true));
        assert_eq!(v["opcode"], json!(1));
    }

    #[test]
    fn text_format_output() {
        let out = parse("81 05 48 65 6c 6c 6f", "hex", "text").unwrap();
        assert!(out.contains("opcode:  0x1 (text)"), "got: {out}");
        assert!(out.contains("payload (text): Hello"), "got: {out}");
        assert!(out.contains("FIN:     true"), "got: {out}");
    }

    #[test]
    fn base64_input() {
        // base64 of 81 05 48 65 6c 6c 6f
        let out = parse("gQVIZWxsbw==", "base64", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["payload_text"], json!("Hello"));
    }

    #[test]
    fn auto_detects_hex() {
        let out = parse("810548656c6c6f", "auto", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["payload_text"], json!("Hello"));
    }

    #[test]
    fn trailing_bytes_noted() {
        // One ping frame (89 00) followed by extra bytes.
        let out = parse("89 00 81 05 48 65 6c 6c 6f", "hex", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["opcode"], json!(9));
        assert_eq!(v["frame_bytes"], json!(2));
        assert!(v["note"].as_str().unwrap().contains("trailing"));
    }

    #[test]
    fn rejects_empty() {
        let err = parse("", "hex", "json").unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn rejects_too_short_header() {
        let err = parse("81", "hex", "json").unwrap_err();
        assert!(err.contains("at least 2 bytes"), "got: {err}");
    }

    #[test]
    fn rejects_truncated_payload() {
        // Header says 5-byte payload but only 2 bytes follow.
        let err = parse("81 05 48 65", "hex", "json").unwrap_err();
        assert!(err.contains("truncated"), "got: {err}");
    }

    #[test]
    fn rejects_truncated_extended_126() {
        // 0x7e (126) marker but no 2-byte extended length.
        let err = parse("82 7e 01", "hex", "json").unwrap_err();
        assert!(err.contains("126"), "got: {err}");
    }

    #[test]
    fn rejects_truncated_masking_key() {
        // MASK bit set, len 5, but only 2 of 4 key bytes.
        let err = parse("81 85 37 fa", "hex", "json").unwrap_err();
        assert!(err.contains("masking key"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_format() {
        let err = parse("81 00", "hex", "yaml").unwrap_err();
        assert!(err.contains("invalid format"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_encoding() {
        let err = parse("81 00", "octal", "json").unwrap_err();
        assert!(err.contains("invalid encoding"), "got: {err}");
    }
}
