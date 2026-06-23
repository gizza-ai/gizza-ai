//! flask-session-decode core — decode a Flask / itsdangerous session cookie into
//! readable JSON WITHOUT the secret key. No wafer/wasm-bindgen deps; pure-Rust
//! (base64url + zlib via flate2), so it runs on every backend.
//!
//! Flask's default session is set by `flask.sessions.SecureCookieSessionInterface`,
//! which uses itsdangerous `URLSafeTimedSerializer`. The cookie value is three
//! base64url segments joined by '.':  `<payload>.<timestamp>.<signature>`.
//!   - payload: the URL-safe-base64 (no padding) of the session JSON. If the JSON
//!     was zlib-compressed (itsdangerous compresses when it shrinks the output),
//!     the whole payload segment is prefixed with a literal '.' and the rest is
//!     base64url of the zlib stream.
//!   - timestamp: base64url of a big-endian integer = seconds since the itsdangerous
//!     epoch (2011-01-01T00:00:00Z = Unix 1293840000).
//!   - signature: HMAC; we do NOT verify it (no key) and report it verbatim.
//!
//! Decoding is signature-free on purpose: the goal is inspection without the
//! `SECRET_KEY`. The output is informational and unauthenticated.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use flate2::read::ZlibDecoder;
use serde::Serialize;
use std::io::Read;

/// itsdangerous epoch: 2011-01-01T00:00:00Z as seconds since the Unix epoch.
const ITSDANGEROUS_EPOCH: i64 = 1_293_840_000;

#[derive(Serialize, Debug)]
pub struct Decoded {
    /// The decoded session payload, as JSON when it parses, else the raw UTF-8
    /// string (or a note for non-text).
    pub payload: serde_json::Value,
    /// Whether the payload segment was zlib-compressed ('.'-prefixed).
    pub compressed: bool,
    /// The signature timestamp, seconds since the Unix epoch (null if absent).
    pub timestamp: Option<i64>,
    /// The timestamp formatted as an ISO-8601 / RFC-3339 UTC string (null if absent).
    pub timestamp_iso: Option<String>,
    /// The raw signature segment (base64url), unverified — we have no key.
    pub signature: String,
    /// Always false: the signature is not checked (no secret key is used).
    pub signature_verified: bool,
}

fn b64url_decode(seg: &str) -> Result<Vec<u8>, String> {
    URL_SAFE_NO_PAD
        .decode(seg.as_bytes())
        .map_err(|e| format!("segment is not valid base64url: {e}"))
}

/// Decode a big-endian byte slice as an unsigned integer (itsdangerous timestamp).
fn be_bytes_to_i64(bytes: &[u8]) -> i64 {
    let mut n: i64 = 0;
    for &b in bytes {
        n = (n << 8) | (b as i64);
    }
    n
}

/// Format a Unix timestamp (seconds, UTC) as an RFC-3339 string without pulling a
/// date crate. Proleptic Gregorian calendar via Howard Hinnant's civil-from-days.
fn unix_to_iso(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Turn raw decoded payload bytes into a JSON value: parse as JSON if possible,
/// else expose the UTF-8 text, else describe it as binary.
fn payload_to_value(bytes: &[u8]) -> serde_json::Value {
    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(bytes) {
        return v;
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => serde_json::Value::String(s.to_string()),
        Err(_) => serde_json::Value::String(format!("<{} bytes of non-JSON binary>", bytes.len())),
    }
}

/// Decode a Flask/itsdangerous session cookie value into structured data.
///
/// Accepts the bare cookie value, optionally prefixed with `session=` and/or
/// surrounded by whitespace/quotes (so a pasted `Set-Cookie`/`Cookie` fragment
/// works).
pub fn decode(input: &str) -> Result<Decoded, String> {
    let cookie = normalize_cookie(input)?;

    let parts: Vec<&str> = cookie.split('.').collect();
    // A signed itsdangerous value is payload.timestamp.signature → at least 3
    // dot-separated pieces. A compressed payload begins with a leading '.', which
    // produces an empty first piece, so handle that explicitly.
    let (payload_seg, compressed, rest): (&str, bool, &[&str]) = if cookie.starts_with('.') {
        // ".<b64>.<ts>.<sig>" → parts = ["", "<b64>", "<ts>", "<sig>"]
        if parts.len() < 4 {
            return Err("compressed cookie must be .payload.timestamp.signature".into());
        }
        (parts[1], true, &parts[2..])
    } else {
        if parts.len() < 3 {
            return Err(
                "not a signed session cookie: expected payload.timestamp.signature".into(),
            );
        }
        (parts[0], false, &parts[1..])
    };

    // rest[0] = timestamp, rest[1..] joined = signature (itsdangerous re-joins the
    // signature with '.'; in practice the HMAC is one segment, but be tolerant).
    let timestamp_seg = rest[0];
    let signature = rest[1..].join(".");

    let payload_bytes = b64url_decode(payload_seg)?;
    let payload_bytes = if compressed {
        let mut dec = ZlibDecoder::new(&payload_bytes[..]);
        let mut out = Vec::new();
        dec.read_to_end(&mut out)
            .map_err(|e| format!("payload zlib decompression failed: {e}"))?;
        out
    } else {
        payload_bytes
    };
    let payload = payload_to_value(&payload_bytes);

    let (timestamp, timestamp_iso) = if timestamp_seg.is_empty() {
        (None, None)
    } else {
        let ts_bytes = b64url_decode(timestamp_seg)?;
        // itsdangerous timestamps are offset from its own epoch (2011-01-01).
        let unix = be_bytes_to_i64(&ts_bytes) + ITSDANGEROUS_EPOCH;
        (Some(unix), Some(unix_to_iso(unix)))
    };

    Ok(Decoded {
        payload,
        compressed,
        timestamp,
        timestamp_iso,
        signature,
        signature_verified: false,
    })
}

/// Strip a leading `session=` (or other `name=`) cookie key, whitespace, a trailing
/// `;`-attribute list, and surrounding quotes from a pasted cookie fragment.
fn normalize_cookie(input: &str) -> Result<String, String> {
    let mut s = input.trim();
    if s.is_empty() {
        return Err("empty input: paste a Flask session cookie value".into());
    }
    // Drop everything after the first ';' (cookie attributes / other cookies).
    if let Some(idx) = s.find(';') {
        s = s[..idx].trim_end();
    }
    // Strip a leading "name=" (e.g. "session=") if present and token-shaped.
    if let Some(eq) = s.find('=') {
        let name = &s[..eq];
        if !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        {
            s = s[eq + 1..].trim();
        }
    }
    let s = s.trim_matches('"').trim();
    if s.is_empty() {
        return Err("empty cookie value after stripping name/quotes".into());
    }
    Ok(s.to_string())
}

/// Decode and serialize to a pretty JSON string (the surface-facing entry point).
pub fn decode_to_json(input: &str) -> Result<String, String> {
    let decoded = decode(input)?;
    serde_json::to_string_pretty(&decoded).map_err(|e| format!("serialization failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_uncompressed() -> String {
        let payload = br#"{"logged_in":true,"user":"alice"}"#;
        let p = URL_SAFE_NO_PAD.encode(payload);
        let off = 1_700_000_000i64 - ITSDANGEROUS_EPOCH;
        let bytes = off.to_be_bytes();
        let first = bytes.iter().position(|&b| b != 0).unwrap();
        let ts = URL_SAFE_NO_PAD.encode(&bytes[first..]);
        format!("{p}.{ts}.fakeSIGNATURE")
    }

    #[test]
    fn decodes_uncompressed_session() {
        let cookie = make_uncompressed();
        let d = decode(&cookie).unwrap();
        assert!(!d.compressed);
        assert_eq!(d.payload["user"], "alice");
        assert_eq!(d.payload["logged_in"], true);
        assert_eq!(d.timestamp, Some(1_700_000_000));
        assert_eq!(d.timestamp_iso.as_deref(), Some("2023-11-14T22:13:20Z"));
        assert_eq!(d.signature, "fakeSIGNATURE");
        assert!(!d.signature_verified);
    }

    #[test]
    fn decodes_compressed_session() {
        use flate2::write::ZlibEncoder;
        use flate2::Compression;
        use std::io::Write;
        let payload = br#"{"a":1,"b":[1,2,3,4,5,6,7,8,9,10],"c":"hello world hello world"}"#;
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::best());
        enc.write_all(payload).unwrap();
        let compressed = enc.finish().unwrap();
        let p = URL_SAFE_NO_PAD.encode(&compressed);
        let cookie = format!(".{p}.AAAAAA.sig");
        let d = decode(&cookie).unwrap();
        assert!(d.compressed);
        assert_eq!(d.payload["a"], 1);
        assert_eq!(d.payload["b"][9], 10);
    }

    #[test]
    fn strips_session_prefix_and_quotes() {
        let inner = make_uncompressed();
        let wrapped = format!("session=\"{inner}\"; Path=/; HttpOnly; SameSite=Lax");
        let d = decode(&wrapped).unwrap();
        assert_eq!(d.payload["user"], "alice");
    }

    #[test]
    fn non_json_payload_falls_back_to_string() {
        let p = URL_SAFE_NO_PAD.encode(b"not json at all");
        let cookie = format!("{p}.AAAAAA.sig");
        let d = decode(&cookie).unwrap();
        assert_eq!(d.payload, serde_json::Value::String("not json at all".into()));
    }

    #[test]
    fn rejects_empty_input() {
        assert!(decode("").is_err());
        assert!(decode("   ").is_err());
    }

    #[test]
    fn rejects_too_few_segments() {
        let err = decode("onlyonesegment").unwrap_err();
        assert!(err.contains("expected payload.timestamp.signature"));
    }

    #[test]
    fn rejects_bad_base64_payload() {
        assert!(decode("****.AAAAAA.sig").is_err());
    }

    #[test]
    fn epoch_conversion_is_correct() {
        assert_eq!(unix_to_iso(ITSDANGEROUS_EPOCH), "2011-01-01T00:00:00Z");
        assert_eq!(unix_to_iso(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn decode_to_json_pretty_works() {
        let cookie = make_uncompressed();
        let json = decode_to_json(&cookie).unwrap();
        assert!(json.contains("\"user\": \"alice\""));
        assert!(json.contains("\"compressed\": false"));
    }
}
