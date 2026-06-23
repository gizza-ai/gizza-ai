//! ja4-server-fingerprint core — compute the JA4S server fingerprint from a raw
//! TLS ServerHello given as a hex string. Pure-Rust, no wafer/wasm-bindgen deps.
//!
//! JA4S (FoxIO, 2023) is the server half of the JA4+ suite. It fingerprints the
//! server's TLS response by combining fields from the ServerHello:
//!
//!   JA4S = (ptype)(version)(ext_count)(alpn) _ (cipher) _ (sha256_of_extensions)
//!
//! Concretely the string is `a_b_c` where:
//!   a = ptype + version + ext_count(2 digits) + alpn(2 chars)
//!       - ptype: 't' for TCP, 'q' for QUIC.
//!       - version: TLS version 2-char code (13,12,11,10,s3,s2) — taken from the
//!         supported_versions extension if the server sent one, else the
//!         legacy_version field.
//!       - ext_count: number of extensions in the ServerHello, min(count,99),
//!         zero-padded to 2 digits.
//!       - alpn: first+last char of the ALPN protocol the server chose, '00' if
//!         none, '99' if the first byte is non-ASCII (>127).
//!   b = the single chosen cipher suite, 4 lowercase hex chars.
//!   c = first 12 hex chars of SHA256 of the comma-joined extension type list,
//!       each type as 4 lowercase hex chars, in wire order, GREASE kept. If the
//!       ServerHello has no extensions, c = "000000000000".
//!
//! Unlike JA3, JA4S keeps GREASE values in the extension hash and does NOT sort
//! the list — it hashes them in the order they appear on the wire.

use serde::Serialize;
use sha2::{Digest, Sha256};

/// The parsed JA4S result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ja4s {
    /// The full JA4S fingerprint string (`a_b_c`).
    pub ja4s: String,
    /// The "raw" JA4S where the extension hash is replaced by the comma-joined
    /// hex extension list itself (the `_r` variant in the reference tooling).
    pub ja4s_r: String,
    /// The transport: "TCP" or "QUIC".
    pub transport: String,
    /// The negotiated TLS version, e.g. "TLS 1.3 (0x0304)".
    pub tls_version: String,
    /// The chosen cipher suite as 4 lowercase hex chars, e.g. "c02b".
    pub cipher: String,
    /// The extension types present, in wire order, as 4-char hex (GREASE kept).
    pub extensions: Vec<String>,
    /// The ALPN protocol the server selected, if any (informational).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpn: Option<String>,
}

/// True for a GREASE value per RFC 8701 (0x?a?a where both bytes are equal and
/// of the form 0x_A): 0x0a0a, 0x1a1a, …, 0xfafa.
fn is_grease(v: u16) -> bool {
    (v & 0x0f0f) == 0x0a0a && (v >> 8) == (v & 0xff)
}

/// Strip whitespace and common separators / a 0x prefix, then hex-decode.
fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let mut cleaned = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            ' ' | '\t' | '\n' | '\r' | ':' | '-' | '.' | ',' | '_' => continue,
            _ => cleaned.push(c),
        }
    }
    let cleaned = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
        .unwrap_or(&cleaned);
    if cleaned.is_empty() {
        return Err("no hex bytes found in input".into());
    }
    if cleaned.len() % 2 != 0 {
        return Err("hex input has an odd number of digits".into());
    }
    let mut out = Vec::with_capacity(cleaned.len() / 2);
    let bytes = cleaned.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_val(bytes[i])?;
        let lo = hex_val(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

fn hex_val(b: u8) -> Result<u8, String> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(format!("invalid hex digit '{}'", b as char)),
    }
}

/// Map a 16-bit TLS version to the JA4 2-char code, or "00" if unknown.
fn ja4_version_code(v: u16) -> &'static str {
    match v {
        0x0002 => "s2",
        0x0300 => "s3",
        0x0301 => "10",
        0x0302 => "11",
        0x0303 => "12",
        0x0304 => "13",
        _ => "00",
    }
}

fn version_name(v: u16) -> String {
    let name = match v {
        0x0002 => "SSL 2.0",
        0x0300 => "SSL 3.0",
        0x0301 => "TLS 1.0",
        0x0302 => "TLS 1.1",
        0x0303 => "TLS 1.2",
        0x0304 => "TLS 1.3",
        _ => "Unknown",
    };
    format!("{} (0x{:04x})", name, v)
}

/// A tiny cursor over a byte slice with bounds-checked big-endian reads.
struct Cur<'a> {
    b: &'a [u8],
    i: usize,
}
impl<'a> Cur<'a> {
    fn new(b: &'a [u8]) -> Self {
        Cur { b, i: 0 }
    }
    fn remaining(&self) -> usize {
        self.b.len() - self.i
    }
    fn u8(&mut self) -> Result<u8, String> {
        if self.remaining() < 1 {
            return Err("unexpected end of ServerHello".into());
        }
        let v = self.b[self.i];
        self.i += 1;
        Ok(v)
    }
    fn u16(&mut self) -> Result<u16, String> {
        if self.remaining() < 2 {
            return Err("unexpected end of ServerHello".into());
        }
        let v = ((self.b[self.i] as u16) << 8) | (self.b[self.i + 1] as u16);
        self.i += 2;
        Ok(v)
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.remaining() < n {
            return Err("unexpected end of ServerHello".into());
        }
        let s = &self.b[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }
}

/// Compute the JA4S fingerprint from raw ServerHello bytes. The input may begin
/// at an optional 5-byte TLS record header (content type 0x16) and/or the
/// 4-byte handshake header (type 0x02 ServerHello + 3-byte length); both are
/// detected and skipped so callers can paste the record, the handshake message,
/// or the ServerHello body. `quic` selects the 'q' transport prefix.
pub fn compute(bytes: &[u8], quic: bool) -> Result<Ja4s, String> {
    if bytes.len() < 4 {
        return Err("input is too short to be a ServerHello".into());
    }
    let mut start = 0usize;
    // Optional TLS record header: content_type(22=handshake) version(2) length(2).
    if bytes[start] == 0x16 && bytes.len() >= start + 5 {
        start += 5;
    }
    // Handshake header: msg_type(2=ServerHello) length(3).
    if bytes.len() >= start + 1 && bytes[start] == 0x02 {
        start += 4; // skip type + 3-byte length
    }
    if start >= bytes.len() {
        return Err("input is too short to be a ServerHello".into());
    }

    let mut c = Cur::new(&bytes[start..]);

    // legacy_version (2)
    let legacy_version = c.u16()?;
    // random (32)
    c.take(32)?;
    // session_id
    let sid_len = c.u8()? as usize;
    c.take(sid_len)?;
    // cipher_suite (a single suite in the ServerHello)
    let cipher = c.u16()?;
    // compression_method (1)
    let _comp = c.u8()?;

    let mut extensions: Vec<String> = Vec::new();
    let mut selected_version: Option<u16> = None;
    let mut alpn: Option<String> = None;

    // extensions (optional in older TLS, present in real ServerHellos)
    if c.remaining() >= 2 {
        let ext_total = c.u16()? as usize;
        if ext_total > c.remaining() {
            return Err("extensions length exceeds available bytes".into());
        }
        let end = c.i + ext_total;
        while c.i < end {
            let ext_type = c.u16()?;
            let ext_len = c.u16()? as usize;
            let ext_data = c.take(ext_len)?;
            extensions.push(format!("{:04x}", ext_type));
            match ext_type {
                // supported_versions: in the ServerHello this carries the single
                // selected version (2 bytes), not a list.
                0x002b => {
                    if ext_data.len() >= 2 {
                        let v = ((ext_data[0] as u16) << 8) | (ext_data[1] as u16);
                        if !is_grease(v) {
                            selected_version = Some(v);
                        }
                    }
                }
                // application_layer_protocol_negotiation (ALPN): the server
                // returns a single chosen protocol.
                0x0010 => {
                    // list_len(2) [ proto_len(1) proto ]
                    if ext_data.len() >= 3 {
                        let plen = ext_data[2] as usize;
                        if 3 + plen <= ext_data.len() {
                            alpn = Some(
                                String::from_utf8_lossy(&ext_data[3..3 + plen]).into_owned(),
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Version: prefer the supported_versions selection, else legacy_version.
    let effective_version = selected_version.unwrap_or(legacy_version);
    let version_code = ja4_version_code(effective_version);

    // ext_count: min(count, 99), zero-padded to 2 digits.
    let ext_count = extensions.len().min(99);

    // ALPN 2-char field.
    let alpn_field = match &alpn {
        None => "00".to_string(),
        Some(s) if s.is_empty() => "00".to_string(),
        Some(s) => {
            let b = s.as_bytes();
            if b[0] > 127 {
                "99".to_string()
            } else {
                let first = b[0] as char;
                let last = *b.last().unwrap() as char;
                format!("{}{}", first, last)
            }
        }
    };

    let ptype = if quic { 'q' } else { 't' };
    let cipher_hex = format!("{:04x}", cipher);

    // Extension hash: SHA256 of the comma-joined hex extension list (wire order,
    // GREASE kept), first 12 hex chars; "000000000000" when there are none.
    let ext_hash = if extensions.is_empty() {
        "000000000000".to_string()
    } else {
        let joined = extensions.join(",");
        let digest = Sha256::digest(joined.as_bytes());
        let mut hex = String::with_capacity(64);
        for b in digest.iter() {
            hex.push_str(&format!("{:02x}", b));
        }
        hex[..12].to_string()
    };

    let a = format!("{}{}{:02}{}", ptype, version_code, ext_count, alpn_field);
    let ja4s = format!("{}_{}_{}", a, cipher_hex, ext_hash);
    let ja4s_r = format!("{}_{}_{}", a, cipher_hex, extensions.join(","));

    Ok(Ja4s {
        ja4s,
        ja4s_r,
        transport: if quic { "QUIC".into() } else { "TCP".into() },
        tls_version: version_name(effective_version),
        cipher: cipher_hex,
        extensions,
        alpn,
    })
}

/// Decode a hex ServerHello and compute its JA4S fingerprint (TCP transport).
pub fn run(input: &str) -> Result<Ja4s, String> {
    run_with(input, false)
}

/// Decode a hex ServerHello and compute its JA4S fingerprint, choosing the
/// transport (false = TCP → 't', true = QUIC → 'q').
pub fn run_with(input: &str, quic: bool) -> Result<Ja4s, String> {
    let bytes = decode_hex(input)?;
    compute(&bytes, quic)
}

/// JSON-string entry point used by the web page wrapper.
pub fn render(input: &str, quic: bool) -> Result<String, String> {
    let j = run_with(input, quic)?;
    serde_json::to_string_pretty(&j).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // A ServerHello (record + handshake headers):
    //   legacy_version 0x0303, cipher c02b, no session id,
    //   extensions: supported_versions(002b)->0304, key_share(0033), alpn(0010)->h2.
    fn sample_hex() -> &'static str {
        "16030300630200005f0303000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f00c02b000037002b0002030400330024001d00200000000000000000000000000000000000000000000000000000000000000000001000050003026832"
    }

    #[test]
    fn computes_ja4s_tls13() {
        let r = run(sample_hex()).expect("should parse");
        // version: supported_versions 0304 -> 13; 3 extensions; alpn h2 -> "h2"
        // cipher c02b; ext hash of "002b,0033,0010".
        assert_eq!(r.ja4s, "t1303h2_c02b_19fd10492780");
        assert_eq!(r.ja4s_r, "t1303h2_c02b_002b,0033,0010");
        assert_eq!(r.transport, "TCP");
        assert_eq!(r.tls_version, "TLS 1.3 (0x0304)");
        assert_eq!(r.cipher, "c02b");
        assert_eq!(r.extensions, vec!["002b", "0033", "0010"]);
        assert_eq!(r.alpn, Some("h2".to_string()));
    }

    #[test]
    fn quic_transport_prefix() {
        let r = run_with(sample_hex(), true).unwrap();
        assert!(r.ja4s.starts_with("q1303h2_"));
        assert_eq!(r.transport, "QUIC");
    }

    #[test]
    fn tls12_no_extensions() {
        // legacy_version 0303 (no supported_versions), cipher 009c, no extensions.
        let hex = "160303002a020000260303000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f00009c00";
        let r = run(hex).unwrap();
        // t + version 12 + extcount 00 + alpn 00 = "t120000" ; cipher 009c ; empty hash.
        assert_eq!(r.ja4s, "t120000_009c_000000000000");
        assert_eq!(r.tls_version, "TLS 1.2 (0x0303)");
        assert_eq!(r.cipher, "009c");
        assert!(r.extensions.is_empty());
        assert_eq!(r.alpn, None);
    }

    #[test]
    fn alpn_none_when_not_selected() {
        // TLS 1.3 via supported_versions but no ALPN extension. alpn field -> "00".
        // extensions: supported_versions(002b)->0304 only. ext_count 01, cipher 1301.
        let hex = "16030300320200002e0303000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f001301000006002b00020304";
        let r = run(hex).unwrap();
        assert_eq!(r.ja4s, "t130100_1301_b9a491fefe05");
        assert_eq!(r.tls_version, "TLS 1.3 (0x0304)");
        assert_eq!(r.cipher, "1301");
        assert_eq!(r.extensions, vec!["002b"]);
        assert_eq!(r.alpn, None);
    }

    #[test]
    fn rejects_garbage() {
        assert!(run("zzzz").is_err());
        assert!(run("16").is_err()); // too short
        assert!(run("").is_err());
        assert!(run("abc").is_err()); // odd digits
    }

    #[test]
    fn accepts_separators_and_prefix() {
        let r = run(&format!("0x{}", sample_hex())).unwrap();
        assert_eq!(r.ja4s, "t1303h2_c02b_19fd10492780");
    }

    #[test]
    fn grease_detection() {
        assert!(is_grease(0x0a0a));
        assert!(is_grease(0xfafa));
        assert!(!is_grease(0xc02b));
        assert!(!is_grease(0x002b));
    }

    #[test]
    fn version_codes() {
        assert_eq!(ja4_version_code(0x0304), "13");
        assert_eq!(ja4_version_code(0x0303), "12");
        assert_eq!(ja4_version_code(0x0301), "10");
        assert_eq!(ja4_version_code(0x0300), "s3");
        assert_eq!(ja4_version_code(0x0fff), "00");
    }

    #[test]
    fn render_emits_json() {
        let s = render(sample_hex(), false).unwrap();
        assert!(s.contains("\"ja4s\""));
        assert!(s.contains("t1303h2_c02b_19fd10492780"));
        assert!(s.contains("\"cipher\""));
    }
}
