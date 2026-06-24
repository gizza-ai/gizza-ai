//! ja3-fingerprint core — compute the JA3 TLS client fingerprint from a raw
//! ClientHello given as a hex string. Pure-Rust, no wafer/wasm-bindgen deps.
//!
//! JA3 (Salesforce, 2017) fingerprints a TLS client by concatenating five
//! decimal fields from its ClientHello, joined in this exact order:
//!
//!   SSLVersion,Ciphers,Extensions,EllipticCurves,EllipticCurvePointFormats
//!
//! - SSLVersion: the ClientHello legacy_version (e.g. 771 = 0x0303 = TLS 1.2)
//! - Ciphers: the offered cipher suites, decimal, joined by "-"
//! - Extensions: the extension types, decimal, joined by "-"
//! - EllipticCurves: the supported_groups (ext 10) named-group values
//! - EllipticCurvePointFormats: the ec_point_formats (ext 11) values
//!
//! Each list omits GREASE values (RFC 8701: 0x0a0a, 0x1a1a, … 0xfafa). The
//! MD5 of the resulting string is the JA3 hash you'll see in IDS/proxy logs.

use serde::Serialize;

/// The parsed JA3 result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ja3 {
    /// The full JA3 string (the five comma-joined fields).
    pub ja3: String,
    /// The MD5 hash of the JA3 string (32 lowercase hex chars).
    pub ja3_md5: String,
    /// The JA3N (normalized) string — identical to JA3 except the extension
    /// list is sorted ascending, so the fingerprint is stable across the TLS
    /// extension-order randomization used by modern browsers (Chrome 110+,
    /// Firefox 114+).
    pub ja3n: String,
    /// The MD5 hash of the JA3N string (32 lowercase hex chars).
    pub ja3n_md5: String,
    /// The ClientHello legacy_version, e.g. "TLS 1.2 (0x0303)".
    pub tls_version: String,
    /// Offered cipher suites (GREASE removed) as decimal numbers.
    pub ciphers: Vec<u16>,
    /// Extension types present (GREASE removed) as decimal numbers.
    pub extensions: Vec<u16>,
    /// supported_groups / elliptic curves (GREASE removed) as decimal numbers.
    pub elliptic_curves: Vec<u16>,
    /// ec_point_formats as decimal numbers.
    pub elliptic_curve_point_formats: Vec<u8>,
    /// SNI server name(s) carried in the ClientHello, if any (informational).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub server_names: Vec<String>,
}

/// True for a GREASE value per RFC 8701 (the 0x?A?A reserved pattern, where
/// both bytes are equal and of the form 0x_A): 0x0a0a, 0x1a1a, …, 0xfafa.
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
    // Drop a single leading 0x / 0X if present.
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

fn version_name(v: u16) -> String {
    let name = match v {
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
            return Err("unexpected end of ClientHello".into());
        }
        let v = self.b[self.i];
        self.i += 1;
        Ok(v)
    }
    fn u16(&mut self) -> Result<u16, String> {
        if self.remaining() < 2 {
            return Err("unexpected end of ClientHello".into());
        }
        let v = ((self.b[self.i] as u16) << 8) | (self.b[self.i + 1] as u16);
        self.i += 2;
        Ok(v)
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.remaining() < n {
            return Err("unexpected end of ClientHello".into());
        }
        let s = &self.b[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }
}

/// Parse the bytes into a JA3 fingerprint. The input may begin with an
/// optional 5-byte TLS record header (content type 0x16) and/or the 4-byte
/// handshake header (type 0x01 ClientHello + 3-byte length); both are detected
/// and skipped so callers can paste either the record, the handshake message,
/// or the ClientHello body.
pub fn compute(bytes: &[u8]) -> Result<Ja3, String> {
    if bytes.len() < 4 {
        return Err("input is too short to be a ClientHello".into());
    }
    let mut start = 0usize;
    // Optional TLS record header: content_type(22=handshake) version(2) length(2).
    if bytes[start] == 0x16 && bytes.len() >= start + 5 {
        start += 5;
    }
    // Handshake header: msg_type(1=ClientHello) length(3).
    if bytes.len() >= start + 1 && bytes[start] == 0x01 {
        start += 4; // skip type + 3-byte length
    }
    if start >= bytes.len() {
        return Err("input is too short to be a ClientHello".into());
    }

    let mut c = Cur::new(&bytes[start..]);

    // legacy_version (2)
    let version = c.u16()?;
    // random (32)
    c.take(32)?;
    // session_id
    let sid_len = c.u8()? as usize;
    c.take(sid_len)?;
    // cipher_suites
    let cs_len = c.u16()? as usize;
    if cs_len % 2 != 0 {
        return Err("cipher suites length is not a multiple of 2".into());
    }
    let cs_bytes = c.take(cs_len)?;
    let mut ciphers = Vec::new();
    for pair in cs_bytes.chunks_exact(2) {
        let v = ((pair[0] as u16) << 8) | (pair[1] as u16);
        if !is_grease(v) {
            ciphers.push(v);
        }
    }
    // compression_methods
    let comp_len = c.u8()? as usize;
    c.take(comp_len)?;

    let mut extensions = Vec::new();
    let mut elliptic_curves = Vec::new();
    let mut elliptic_curve_point_formats = Vec::new();
    let mut server_names = Vec::new();

    // extensions (optional in SSL3, present in real-world ClientHellos)
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
            if !is_grease(ext_type) {
                extensions.push(ext_type);
            }
            match ext_type {
                0x000a => {
                    // supported_groups: 2-byte list length, then u16 groups.
                    if ext_data.len() >= 2 {
                        let list_len = ((ext_data[0] as usize) << 8) | (ext_data[1] as usize);
                        let body = &ext_data[2..];
                        let take = list_len.min(body.len());
                        for pair in body[..take].chunks_exact(2) {
                            let g = ((pair[0] as u16) << 8) | (pair[1] as u16);
                            if !is_grease(g) {
                                elliptic_curves.push(g);
                            }
                        }
                    }
                }
                0x000b => {
                    // ec_point_formats: 1-byte list length, then u8 formats.
                    if !ext_data.is_empty() {
                        let list_len = ext_data[0] as usize;
                        let body = &ext_data[1..];
                        let take = list_len.min(body.len());
                        for &f in &body[..take] {
                            elliptic_curve_point_formats.push(f);
                        }
                    }
                }
                0x0000 => {
                    // server_name: list_len(2) [ name_type(1) name_len(2) name ]*
                    if ext_data.len() >= 2 {
                        let mut j = 2usize;
                        while j + 3 <= ext_data.len() {
                            let name_type = ext_data[j];
                            let nlen =
                                ((ext_data[j + 1] as usize) << 8) | (ext_data[j + 2] as usize);
                            j += 3;
                            if j + nlen > ext_data.len() {
                                break;
                            }
                            if name_type == 0 {
                                if let Ok(s) = std::str::from_utf8(&ext_data[j..j + nlen]) {
                                    server_names.push(s.to_string());
                                }
                            }
                            j += nlen;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let ja3_str = format!(
        "{},{},{},{},{}",
        version,
        join_dash_u16(&ciphers),
        join_dash_u16(&extensions),
        join_dash_u16(&elliptic_curves),
        join_dash_u8(&elliptic_curve_point_formats),
    );
    let ja3_md5 = md5_hex(ja3_str.as_bytes());

    // JA3N: the normalized variant sorts the extension list ascending so the
    // fingerprint survives TLS extension-order randomization.
    let mut sorted_exts = extensions.clone();
    sorted_exts.sort_unstable();
    let ja3n_str = format!(
        "{},{},{},{},{}",
        version,
        join_dash_u16(&ciphers),
        join_dash_u16(&sorted_exts),
        join_dash_u16(&elliptic_curves),
        join_dash_u8(&elliptic_curve_point_formats),
    );
    let ja3n_md5 = md5_hex(ja3n_str.as_bytes());

    Ok(Ja3 {
        ja3: ja3_str,
        ja3_md5,
        ja3n: ja3n_str,
        ja3n_md5,
        tls_version: version_name(version),
        ciphers,
        extensions,
        elliptic_curves,
        elliptic_curve_point_formats,
        server_names,
    })
}

fn join_dash_u16(v: &[u16]) -> String {
    v.iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join("-")
}
fn join_dash_u8(v: &[u8]) -> String {
    v.iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join("-")
}

fn md5_hex(data: &[u8]) -> String {
    let digest = md5::compute(data);
    let mut s = String::with_capacity(32);
    for b in digest.0.iter() {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Decode a hex ClientHello and compute its JA3 fingerprint.
pub fn run(input: &str) -> Result<Ja3, String> {
    let bytes = decode_hex(input)?;
    compute(&bytes)
}

/// JSON-string entry point used by the web page wrapper.
pub fn render(input: &str) -> Result<String, String> {
    let j = run(input)?;
    serde_json::to_string_pretty(&j).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal, structurally-correct ClientHello (record + handshake headers):
    //   version 0x0303, ciphers GREASE(0a0a) 1301 c02b, comp 00,
    //   ext SNI "gizza.ai", supported_groups GREASE(0a0a) 001d 0017,
    //   ec_point_formats 00, GREASE extension 1a1a.
    fn sample_hex() -> &'static str {
        "160301005a010000560303abababababababababababababababababababababababababababababababab0000060a0a1301c02b010000270000000d000b00000867697a7a612e6169000a000800060a0a001d0017000b000201001a1a0000"
    }

    #[test]
    fn computes_ja3_string_and_md5() {
        let r = run(sample_hex()).expect("should parse");
        // version 0x0303 = 771
        // ciphers: GREASE removed -> 4865(0x1301), 49195(0xc02b)
        // extensions: 0, 10, 11 (GREASE 0x1a1a removed)
        // curves: GREASE removed -> 29, 23
        // point formats: 0
        assert_eq!(r.ja3, "771,4865-49195,0-10-11,29-23,0");
        // extensions already ascending here, so JA3N equals JA3.
        assert_eq!(r.ja3n, "771,4865-49195,0-10-11,29-23,0");
        assert_eq!(r.ja3n_md5, r.ja3_md5);
        assert_eq!(r.tls_version, "TLS 1.2 (0x0303)");
        assert_eq!(r.ciphers, vec![4865, 49195]);
        assert_eq!(r.extensions, vec![0, 10, 11]);
        assert_eq!(r.elliptic_curves, vec![29, 23]);
        assert_eq!(r.elliptic_curve_point_formats, vec![0]);
        assert_eq!(r.server_names, vec!["gizza.ai".to_string()]);
        // MD5 of the JA3 string.
        assert_eq!(r.ja3_md5, md5_hex(r.ja3.as_bytes()));
        assert_eq!(r.ja3_md5.len(), 32);
    }

    #[test]
    fn ja3n_sorts_extensions() {
        // Same offered ext set but in descending order (11, 10, 0). JA3 keeps
        // the wire order; JA3N sorts to 0-10-11 (matching the first sample).
        let hex = "16030100520100004e0303cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd0000041301c02b01000021000b00020100000a00060004001d00170000000d000b00000867697a7a612e6169";
        let r = run(hex).unwrap();
        assert_eq!(r.ja3, "771,4865-49195,11-10-0,29-23,0");
        assert_eq!(r.ja3_md5, "0154651b82ee193c0a7a4aacc843c46d");
        assert_eq!(r.ja3n, "771,4865-49195,0-10-11,29-23,0");
        assert_eq!(r.ja3n_md5, "3e916670429427a5a33c947802616cdc");
        assert_ne!(r.ja3_md5, r.ja3n_md5);
    }

    #[test]
    fn md5_of_known_vectors() {
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn grease_detection() {
        assert!(is_grease(0x0a0a));
        assert!(is_grease(0x1a1a));
        assert!(is_grease(0xfafa));
        assert!(!is_grease(0x1301));
        assert!(!is_grease(0x0a0b));
        assert!(!is_grease(0x000a));
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
        assert_eq!(r.ja3, "771,4865-49195,0-10-11,29-23,0");
    }

    #[test]
    fn render_emits_json() {
        let s = render(sample_hex()).unwrap();
        assert!(s.contains("\"ja3\""));
        assert!(s.contains("\"ja3_md5\""));
        assert!(s.contains("771,4865-49195,0-10-11,29-23,0"));
    }
}
