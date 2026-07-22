//! otpauth-migration-decoder core — decode a Google Authenticator
//! `otpauth-migration://offline?data=...` bulk-export payload into individual
//! `otpauth://` provisioning URIs (one per account). Pure Rust, no I/O; the
//! Google-Authenticator migration protobuf is parsed by hand (varint + wire-type
//! reader) so the crate needs no protobuf codegen crate. Every 2FA secret stays
//! on-device.

use serde_json::json;

/// One decoded account (OtpParameters message inside the migration payload).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// OTP type as it appears in the URI scheme path (`totp` or `hotp`).
    pub otp_type: &'static str,
    /// Issuer / provider name (may be empty).
    pub issuer: String,
    /// Account name / label (may be empty).
    pub name: String,
    /// Secret, RFC 4648 base32 encoded, uppercase, no padding.
    pub secret: String,
    /// HMAC algorithm label (`SHA1`, `SHA256`, `SHA512`, `MD5`).
    pub algorithm: &'static str,
    /// Number of OTP digits (6 or 8).
    pub digits: u32,
    /// HOTP counter (0 for TOTP).
    pub counter: u64,
    /// Rebuilt standard `otpauth://` provisioning URI.
    pub uri: String,
}

/// Decode a payload and render the result in the requested `format`
/// (`uri` → one URI per line, `json` → pretty JSON array).
pub fn run_with_format(payload: &str, format: &str) -> Result<String, String> {
    let accounts = decode(payload)?;
    match format.trim().to_ascii_lowercase().as_str() {
        "" | "uri" => Ok(accounts.iter().map(|a| a.uri.clone()).collect::<Vec<_>>().join("\n")),
        "json" => {
            let arr: Vec<serde_json::Value> = accounts
                .iter()
                .map(|a| {
                    json!({
                        "type": a.otp_type,
                        "issuer": a.issuer,
                        "name": a.name,
                        "secret": a.secret,
                        "algorithm": a.algorithm,
                        "digits": a.digits,
                        "counter": a.counter,
                        "uri": a.uri,
                    })
                })
                .collect();
            serde_json::to_string_pretty(&arr).map_err(|e| format!("failed to render JSON: {e}"))
        }
        other => Err(format!("unknown format '{other}' (use uri or json)")),
    }
}

/// Default entry point (URI output).
pub fn run(payload: &str) -> Result<String, String> {
    run_with_format(payload, "uri")
}

/// Decode a migration payload (full URI or bare `data` value) into accounts.
pub fn decode(payload: &str) -> Result<Vec<Account>, String> {
    let data = extract_data(payload)?;
    let decoded = percent_decode(&data);
    let bytes = decode_base64(&decoded)?;
    if bytes.is_empty() {
        return Err("payload decoded to zero bytes — is the data value complete?".into());
    }
    parse_payload(&bytes)
}

/// Pull the base64 payload out of either a full `otpauth-migration://offline?data=...`
/// URI or a bare `data` value pasted on its own.
fn extract_data(payload: &str) -> Result<String, String> {
    let p = payload.trim();
    if p.is_empty() {
        return Err("payload is required — paste an otpauth-migration:// link or its data value".into());
    }
    // A full/partial URI carries the payload in a `data=` query parameter.
    if let Some(idx) = p.find("data=") {
        let rest = &p[idx + "data=".len()..];
        let end = rest.find('&').unwrap_or(rest.len());
        let value = rest[..end].trim();
        if value.is_empty() {
            return Err("the data parameter is empty".into());
        }
        return Ok(value.to_string());
    }
    if p.contains("://") {
        return Err("could not find a data= parameter in the otpauth-migration:// URI".into());
    }
    Ok(p.to_string())
}

/// Decode `%XX` percent-escapes; leave everything else (including `+`, `-`, `_`,
/// `/`) untouched so both base64 and base64url payloads survive intact.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex_val(bytes[i + 1]);
            let lo = hex_val(bytes[i + 2]);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push(hi << 4 | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    // The decoded bytes are still ASCII base64 (percent-escaped bytes were ASCII too).
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Decode base64 or base64url, tolerating missing padding and interior
/// whitespace. Both `+/` and `-_` alphabets are accepted.
fn decode_base64(s: &str) -> Result<Vec<u8>, String> {
    let mut bits: u32 = 0;
    let mut nbits: u32 = 0;
    let mut out = Vec::new();
    for c in s.bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        let v = b64_val(c)
            .ok_or_else(|| format!("malformed base64: unexpected character '{}'", c as char))?;
        bits = (bits << 6) | v as u32;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push((bits >> nbits) as u8);
        }
    }
    Ok(out)
}

fn b64_val(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    }
}

const B32_ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// RFC 4648 base32 encode, uppercase, no `=` padding (authenticator convention).
fn base32_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(5) * 8);
    let mut bits: u32 = 0;
    let mut nbits: u32 = 0;
    for &b in data {
        bits = (bits << 8) | b as u32;
        nbits += 8;
        while nbits >= 5 {
            nbits -= 5;
            out.push(B32_ALPHABET[((bits >> nbits) & 0x1f) as usize] as char);
        }
    }
    if nbits > 0 {
        out.push(B32_ALPHABET[((bits << (5 - nbits)) & 0x1f) as usize] as char);
    }
    out
}

/// Percent-encode keeping the RFC 3986 unreserved set literal (space → `%20`).
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

// ---- protobuf reader -------------------------------------------------------

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }
    fn eof(&self) -> bool {
        self.pos >= self.buf.len()
    }
    fn read_varint(&mut self) -> Result<u64, String> {
        let mut result: u64 = 0;
        let mut shift: u32 = 0;
        loop {
            if self.pos >= self.buf.len() {
                return Err("truncated protobuf: varint ran past end of data".into());
            }
            let b = self.buf[self.pos];
            self.pos += 1;
            result |= ((b & 0x7f) as u64) << shift;
            if b & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift >= 64 {
                return Err("malformed protobuf: varint is too long".into());
            }
        }
        Ok(result)
    }
    fn read_len_delimited(&mut self) -> Result<&'a [u8], String> {
        let len = self.read_varint()? as usize;
        if self.pos + len > self.buf.len() {
            return Err("truncated protobuf: length-delimited field runs past end of data".into());
        }
        let slice = &self.buf[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }
    fn skip(&mut self, wire: u8) -> Result<(), String> {
        match wire {
            0 => {
                self.read_varint()?;
            }
            1 => {
                if self.pos + 8 > self.buf.len() {
                    return Err("truncated protobuf: 64-bit field runs past end of data".into());
                }
                self.pos += 8;
            }
            2 => {
                self.read_len_delimited()?;
            }
            5 => {
                if self.pos + 4 > self.buf.len() {
                    return Err("truncated protobuf: 32-bit field runs past end of data".into());
                }
                self.pos += 4;
            }
            other => return Err(format!("unsupported protobuf wire type {other}")),
        }
        Ok(())
    }
}

/// Parse the top-level MigrationPayload: field 1 is a repeated OtpParameters
/// message; everything else (version, batch metadata) is skipped.
fn parse_payload(bytes: &[u8]) -> Result<Vec<Account>, String> {
    let mut r = Reader::new(bytes);
    let mut accounts = Vec::new();
    while !r.eof() {
        let tag = r.read_varint()?;
        let field = tag >> 3;
        let wire = (tag & 0x7) as u8;
        if field == 1 && wire == 2 {
            let msg = r.read_len_delimited()?;
            accounts.push(parse_otp_parameters(msg)?);
        } else {
            r.skip(wire)?;
        }
    }
    if accounts.is_empty() {
        return Err("no accounts found in the migration payload".into());
    }
    Ok(accounts)
}

fn parse_otp_parameters(bytes: &[u8]) -> Result<Account, String> {
    let mut r = Reader::new(bytes);
    let mut secret: Vec<u8> = Vec::new();
    let mut name = String::new();
    let mut issuer = String::new();
    let mut algorithm: u64 = 1; // ALGO_SHA1
    let mut digits_code: u64 = 1; // DIGIT_COUNT_SIX
    let mut type_code: u64 = 2; // OTP_TYPE_TOTP
    let mut counter: u64 = 0;

    while !r.eof() {
        let tag = r.read_varint()?;
        let field = tag >> 3;
        let wire = (tag & 0x7) as u8;
        match (field, wire) {
            (1, 2) => secret = r.read_len_delimited()?.to_vec(),
            (2, 2) => name = utf8(r.read_len_delimited()?, "name")?,
            (3, 2) => issuer = utf8(r.read_len_delimited()?, "issuer")?,
            (4, 0) => algorithm = r.read_varint()?,
            (5, 0) => digits_code = r.read_varint()?,
            (6, 0) => type_code = r.read_varint()?,
            (7, 0) => counter = r.read_varint()?,
            _ => r.skip(wire)?,
        }
    }

    if secret.is_empty() {
        return Err("an account entry is missing its secret".into());
    }

    let algorithm = match algorithm {
        0 | 1 => "SHA1",
        2 => "SHA256",
        3 => "SHA512",
        4 => "MD5",
        other => return Err(format!("unsupported algorithm code {other} in payload")),
    };
    let digits = match digits_code {
        0 | 1 => 6u32,
        2 => 8u32,
        other => return Err(format!("unsupported digit-count code {other} in payload")),
    };
    let otp_type = match type_code {
        1 => "hotp",
        0 | 2 => "totp",
        other => return Err(format!("unsupported OTP type code {other} in payload")),
    };

    let secret_b32 = base32_encode(&secret);
    let uri = build_uri(otp_type, &issuer, &name, &secret_b32, algorithm, digits, counter);

    Ok(Account {
        otp_type,
        issuer,
        name,
        secret: secret_b32,
        algorithm,
        digits,
        counter,
        uri,
    })
}

fn utf8(bytes: &[u8], what: &str) -> Result<String, String> {
    String::from_utf8(bytes.to_vec()).map_err(|_| format!("{what} field is not valid UTF-8"))
}

/// Build a standard `otpauth://` provisioning URI (Key Uri Format).
fn build_uri(
    otp_type: &str,
    issuer: &str,
    name: &str,
    secret_b32: &str,
    algorithm: &str,
    digits: u32,
    counter: u64,
) -> String {
    let label = if issuer.is_empty() {
        percent_encode(name)
    } else {
        format!("{}:{}", percent_encode(issuer), percent_encode(name))
    };
    let mut query = format!("secret={secret_b32}");
    if !issuer.is_empty() {
        query.push_str(&format!("&issuer={}", percent_encode(issuer)));
    }
    query.push_str(&format!("&algorithm={algorithm}"));
    query.push_str(&format!("&digits={digits}"));
    if otp_type == "hotp" {
        query.push_str(&format!("&counter={counter}"));
    } else {
        query.push_str("&period=30");
    }
    format!("otpauth://{otp_type}/{label}?{query}")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- protobuf encoding helpers (build fixtures) ----
    fn varint(mut v: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut b = (v & 0x7f) as u8;
            v >>= 7;
            if v != 0 {
                b |= 0x80;
            }
            out.push(b);
            if v == 0 {
                break;
            }
        }
        out
    }
    fn tag(field: u64, wire: u8) -> Vec<u8> {
        varint((field << 3) | wire as u64)
    }
    fn ld(field: u64, data: &[u8]) -> Vec<u8> {
        let mut out = tag(field, 2);
        out.extend(varint(data.len() as u64));
        out.extend_from_slice(data);
        out
    }
    fn vf(field: u64, v: u64) -> Vec<u8> {
        let mut out = tag(field, 0);
        out.extend(varint(v));
        out
    }
    fn b64(bytes: &[u8]) -> String {
        // standard base64, no padding — exercises the padding-tolerant decoder.
        const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
            let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
            out.push(A[(n >> 18 & 63) as usize] as char);
            out.push(A[(n >> 12 & 63) as usize] as char);
            if chunk.len() > 1 {
                out.push(A[(n >> 6 & 63) as usize] as char);
            }
            if chunk.len() > 2 {
                out.push(A[(n & 63) as usize] as char);
            }
        }
        out
    }

    /// Two accounts: a TOTP (Example / alice@google.com, SHA1, 6 digits) and an
    /// HOTP (ACME / bob, SHA256, 8 digits, counter 5). Secret bytes "Hello" and
    /// "World" — "Hello" base32-encodes to the well-known "JBSWY3DP".
    fn two_account_payload() -> Vec<u8> {
        let mut acc1 = Vec::new();
        acc1.extend(ld(1, b"Hello")); // secret
        acc1.extend(ld(2, b"alice@google.com")); // name
        acc1.extend(ld(3, b"Example")); // issuer
        acc1.extend(vf(4, 1)); // algo SHA1
        acc1.extend(vf(5, 1)); // digits 6
        acc1.extend(vf(6, 2)); // type TOTP

        let mut acc2 = Vec::new();
        acc2.extend(ld(1, b"World")); // secret
        acc2.extend(ld(2, b"bob")); // name
        acc2.extend(ld(3, b"ACME")); // issuer
        acc2.extend(vf(4, 2)); // algo SHA256
        acc2.extend(vf(5, 2)); // digits 8
        acc2.extend(vf(6, 1)); // type HOTP
        acc2.extend(vf(7, 5)); // counter 5

        let mut payload = Vec::new();
        payload.extend(ld(1, &acc1));
        payload.extend(ld(1, &acc2));
        payload.extend(vf(2, 1)); // version = 1 (skipped)
        payload
    }

    #[test]
    fn base32_hello_is_jbswy3dp() {
        assert_eq!(base32_encode(b"Hello"), "JBSWY3DP");
        assert_eq!(base32_encode(b"World"), "K5XXE3DE");
    }

    #[test]
    fn decodes_multi_account_uris() {
        let data = b64(&two_account_payload());
        let uri = format!("otpauth-migration://offline?data={}", percent_encode(&data));
        let out = run_with_format(&uri, "uri").unwrap();
        assert_eq!(
            out,
            "otpauth://totp/Example:alice%40google.com?secret=JBSWY3DP&issuer=Example&algorithm=SHA1&digits=6&period=30\n\
             otpauth://hotp/ACME:bob?secret=K5XXE3DE&issuer=ACME&algorithm=SHA256&digits=8&counter=5"
        );
    }

    #[test]
    fn bare_data_matches_full_uri() {
        let data = b64(&two_account_payload());
        let full = format!("otpauth-migration://offline?data={}", percent_encode(&data));
        let bare = run_with_format(&data, "uri").unwrap();
        let via_uri = run_with_format(&full, "uri").unwrap();
        assert_eq!(bare, via_uri);
    }

    #[test]
    fn json_format_lists_every_field() {
        let data = b64(&two_account_payload());
        let out = run_with_format(&data, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 2);
        assert_eq!(v[0]["type"], "totp");
        assert_eq!(v[0]["issuer"], "Example");
        assert_eq!(v[0]["name"], "alice@google.com");
        assert_eq!(v[0]["secret"], "JBSWY3DP");
        assert_eq!(v[0]["algorithm"], "SHA1");
        assert_eq!(v[0]["digits"], 6);
        assert_eq!(v[1]["type"], "hotp");
        assert_eq!(v[1]["algorithm"], "SHA256");
        assert_eq!(v[1]["digits"], 8);
        assert_eq!(v[1]["counter"], 5);
        assert!(out.contains('\n'), "JSON should be pretty-printed");
    }

    #[test]
    fn no_issuer_produces_bare_label() {
        let mut acc = Vec::new();
        acc.extend(ld(1, b"Hello"));
        acc.extend(ld(2, b"alice"));
        let mut payload = Vec::new();
        payload.extend(ld(1, &acc));
        let out = run(&b64(&payload)).unwrap();
        assert_eq!(out, "otpauth://totp/alice?secret=JBSWY3DP&algorithm=SHA1&digits=6&period=30");
    }

    #[test]
    fn errors_on_empty_payload() {
        assert!(run("").is_err());
        assert!(run("   ").is_err());
    }

    #[test]
    fn errors_on_malformed_base64() {
        assert!(run("not valid base64 %%%!!!").is_err());
    }

    #[test]
    fn errors_on_truncated_protobuf() {
        let mut bytes = two_account_payload();
        bytes.truncate(bytes.len() - 3);
        let err = parse_payload(&bytes).unwrap_err();
        assert!(err.contains("truncated") || err.contains("past end"), "got: {err}");
    }

    #[test]
    fn errors_on_no_accounts() {
        // A payload with only a version field, no account messages.
        let payload = vf(2, 1);
        let err = parse_payload(&payload).unwrap_err();
        assert!(err.contains("no accounts"), "got: {err}");
    }

    #[test]
    fn errors_on_unknown_format() {
        assert!(run_with_format(&b64(&two_account_payload()), "xml").is_err());
    }
}
