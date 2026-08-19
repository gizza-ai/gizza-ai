//! ssh-public-key-parser core — pure compute, shared by the chat skill block and the web page.
//!
//! Decodes OpenSSH public keys — plain `id_*.pub` lines, `authorized_keys` lines with an
//! options prefix, `known_hosts` entries (including `|1|` hashed hosts and
//! `@cert-authority`/`@revoked` markers), OpenSSH certificates, and RFC 4716 (`---- BEGIN SSH2
//! PUBLIC KEY ----`) blocks — into algorithm, key size, comment and fingerprints.
//!
//! Fingerprints follow `ssh-keygen -l` semantics exactly: they are digests of the binary key
//! blob, and for a *certificate* they are the digest of the certified public key (the blob
//! reconstructed without the certificate envelope), not of the certificate itself.
//!
//! Everything here is pure: no I/O, no clock (the caller passes `now`), no randomness.

use base64::alphabet;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig};
use base64::engine::{DecodePaddingMode, Engine};
use md5::Md5;
use serde::Serialize;
use sha1::Sha1;
use sha2::{Digest, Sha256};

/// Largest accepted input, in bytes. Keeps the 64 MiB wasm sandbox comfortable.
pub const MAX_INPUT_BYTES: usize = 262_144;
/// Largest number of keys parsed from one input.
pub const MAX_KEYS: usize = 200;

/// Padding-tolerant standard base64 (some hand-edited keys drop the trailing `=`).
fn b64() -> GeneralPurpose {
    GeneralPurpose::new(
        &alphabet::STANDARD,
        GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
    )
}

/// Options mirroring the descriptor's optional params.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Also emit the legacy SHA-1 fingerprint (`ssh-keygen -E sha1`).
    pub include_sha1: bool,
    /// Print the MD5 fingerprint's hex digits in uppercase.
    pub uppercase_md5: bool,
    /// Fingerprint to compare every parsed key against (any printed form).
    pub expected_fingerprint: String,
    /// Reference time (Unix seconds) for certificate validity; 0 disables the status fields.
    pub now: i64,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct CertInfo {
    /// `user` or `host` (OpenSSH cert types 1 and 2).
    pub cert_type: String,
    pub serial: u64,
    pub key_id: String,
    /// Empty means the certificate is valid for *any* principal.
    pub principals: Vec<String>,
    pub valid_after: String,
    pub valid_before: String,
    pub valid_after_unix: u64,
    pub valid_before_unix: u64,
    /// `valid`, `expired` or `not_yet_valid` — only when a reference time was supplied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days_until_expiry: Option<i64>,
    pub critical_options: Vec<String>,
    pub extensions: Vec<String>,
    pub ca_algorithm: String,
    pub ca_fingerprint_sha256: String,
    pub signature_algorithm: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct KeyReport {
    /// 1-based position of this key in the input.
    pub index: usize,
    /// `openssh`, `authorized_keys`, `known_hosts` or `rfc4716`.
    pub source_format: String,
    /// The key algorithm exactly as written in the key line.
    pub algorithm: String,
    /// Friendly family name: `RSA`, `Ed25519`, `ECDSA`, `DSA`, `Ed25519 (FIDO)`, …
    pub key_type: String,
    pub is_certificate: bool,
    pub key_size_bits: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub curve: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    pub fingerprint_sha256: String,
    pub fingerprint_md5: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint_sha1: Option<String>,
    /// Size of the decoded binary key blob, in bytes.
    pub blob_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorized_keys_options: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_patterns: Option<Vec<String>>,
    /// `true` when the `known_hosts` entry uses `|1|` HMAC-hashed hostnames.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hashed_hosts: Option<bool>,
    /// `@cert-authority` or `@revoked` marker on a `known_hosts` line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker: Option<String>,
    /// FIDO/security-key application string (`sk-` key types).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fido_application: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate: Option<CertInfo>,
    /// `strong`, `acceptable` or `weak`.
    pub strength: String,
    pub warnings: Vec<String>,
    /// Whether this key matches the supplied `expected_fingerprint`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint_match: Option<bool>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct ParseReport {
    pub key_count: usize,
    pub unique_fingerprints: usize,
    /// `true` when at least one parsed key matches `expected_fingerprint`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_fingerprint_matched: Option<bool>,
    pub keys: Vec<KeyReport>,
}

// ---------------------------------------------------------------------------
// SSH wire-format reader
// ---------------------------------------------------------------------------

struct Reader<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(b: &'a [u8]) -> Self {
        Reader { b, pos: 0 }
    }
    fn remaining(&self) -> usize {
        self.b.len() - self.pos
    }
    fn u32(&mut self, what: &str) -> Result<u32, String> {
        if self.remaining() < 4 {
            return Err(format!(
                "truncated key blob: expected a 4-byte length for {what} at offset {}, only {} byte(s) left",
                self.pos,
                self.remaining()
            ));
        }
        let v = u32::from_be_bytes([
            self.b[self.pos],
            self.b[self.pos + 1],
            self.b[self.pos + 2],
            self.b[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }
    fn u64(&mut self, what: &str) -> Result<u64, String> {
        if self.remaining() < 8 {
            return Err(format!(
                "truncated key blob: expected an 8-byte {what} at offset {}",
                self.pos
            ));
        }
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&self.b[self.pos..self.pos + 8]);
        self.pos += 8;
        Ok(u64::from_be_bytes(buf))
    }
    fn bytes(&mut self, what: &str) -> Result<&'a [u8], String> {
        let len = self.u32(what)? as usize;
        if self.remaining() < len {
            return Err(format!(
                "truncated key blob: {what} claims {len} byte(s) at offset {} but only {} remain",
                self.pos,
                self.remaining()
            ));
        }
        let out = &self.b[self.pos..self.pos + len];
        self.pos += len;
        Ok(out)
    }
    fn string(&mut self, what: &str) -> Result<String, String> {
        let raw = self.bytes(what)?;
        Ok(String::from_utf8_lossy(raw).into_owned())
    }
}

/// Bit length of an SSH `mpint` (two's-complement big-endian, leading zeros stripped).
fn mpint_bits(raw: &[u8]) -> u32 {
    let first = raw.iter().position(|b| *b != 0);
    match first {
        None => 0,
        Some(i) => {
            let significant = &raw[i..];
            (significant.len() as u32 - 1) * 8 + (8 - significant[0].leading_zeros())
        }
    }
}

/// Re-encode an SSH `string` field (4-byte length prefix + payload).
fn wire_string(out: &mut Vec<u8>, payload: &[u8]) {
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
}

// ---------------------------------------------------------------------------
// Fingerprints
// ---------------------------------------------------------------------------

fn b64_nopad(bytes: &[u8]) -> String {
    let s = b64().encode(bytes);
    s.trim_end_matches('=').to_string()
}

fn hex_colons(bytes: &[u8], upper: bool) -> String {
    bytes
        .iter()
        .map(|b| {
            if upper {
                format!("{b:02X}")
            } else {
                format!("{b:02x}")
            }
        })
        .collect::<Vec<_>>()
        .join(":")
}

fn hex_plain(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Timestamps (pure, no chrono)
// ---------------------------------------------------------------------------

/// `forever` sentinel used by OpenSSH for a certificate with no expiry.
const FOREVER: u64 = u64::MAX;
/// Year 9999 in Unix seconds — anything beyond is reported as `forever`.
const MAX_REPRESENTABLE: u64 = 253_402_300_799;

/// Format Unix seconds as `YYYY-MM-DDTHH:MM:SSZ` (Howard Hinnant's civil-from-days).
fn iso8601(secs: u64) -> String {
    if secs >= MAX_REPRESENTABLE {
        return "forever".to_string();
    }
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

// ---------------------------------------------------------------------------
// Key-line scanning
// ---------------------------------------------------------------------------

const PLAIN_ALGOS: &[&str] = &[
    "ssh-rsa",
    "ssh-dss",
    "ssh-ed25519",
    "ssh-ed448",
    "rsa-sha2-256",
    "rsa-sha2-512",
    "ecdsa-sha2-nistp256",
    "ecdsa-sha2-nistp384",
    "ecdsa-sha2-nistp521",
    "sk-ssh-ed25519@openssh.com",
    "sk-ecdsa-sha2-nistp256@openssh.com",
];

const CERT_SUFFIX: &str = "-cert-v01@openssh.com";

fn is_algo_token(tok: &str) -> bool {
    let base = tok.strip_suffix(CERT_SUFFIX).unwrap_or(tok);
    // sk-* certificate names keep the @openssh.com inside the base name.
    let base = if tok.ends_with(CERT_SUFFIX) && tok.starts_with("sk-") {
        match base {
            "sk-ssh-ed25519" => "sk-ssh-ed25519@openssh.com",
            "sk-ecdsa-sha2-nistp256" => "sk-ecdsa-sha2-nistp256@openssh.com",
            other => other,
        }
    } else {
        base
    };
    PLAIN_ALGOS.contains(&base)
}

/// The algorithm name a certificate certifies (`ssh-rsa-cert-v01@openssh.com` → `ssh-rsa`).
fn plain_name_of(algo: &str) -> String {
    match algo.strip_suffix(CERT_SUFFIX) {
        None => algo.to_string(),
        Some("sk-ssh-ed25519") => "sk-ssh-ed25519@openssh.com".to_string(),
        Some("sk-ecdsa-sha2-nistp256") => "sk-ecdsa-sha2-nistp256@openssh.com".to_string(),
        Some(base) => base.to_string(),
    }
}

/// Known `authorized_keys` option keywords — used to tell an options prefix from host patterns.
const AUTH_OPTION_HINTS: &[&str] = &[
    "command=",
    "environment=",
    "from=",
    "permitopen=",
    "permitlisten=",
    "principals=",
    "tunnel=",
    "expiry-time=",
    "restrict",
    "cert-authority",
    "agent-forwarding",
    "port-forwarding",
    "pty",
    "user-rc",
    "x11-forwarding",
    "no-touch-required",
    "verify-required",
    "touch-required",
];

fn looks_like_options(prefix: &str) -> bool {
    let lower = prefix.to_ascii_lowercase();
    AUTH_OPTION_HINTS.iter().any(|h| lower.contains(h))
}

/// Split a comma-separated `authorized_keys` option list, honouring quoted values.
fn split_options(prefix: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut escaped = false;
    for ch in prefix.chars() {
        if escaped {
            cur.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_quotes => {
                cur.push(ch);
                escaped = true;
            }
            '"' => {
                in_quotes = !in_quotes;
                cur.push(ch);
            }
            ',' if !in_quotes => {
                let t = cur.trim();
                if !t.is_empty() {
                    out.push(t.to_string());
                }
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    let t = cur.trim();
    if !t.is_empty() {
        out.push(t.to_string());
    }
    out
}

/// One key line lifted out of the input, before the blob is decoded.
struct Candidate {
    source_format: String,
    algorithm: String,
    body: String,
    comment: Option<String>,
    prefix: Option<String>,
    marker: Option<String>,
}

/// Pull every parseable key line (and RFC 4716 block) out of the raw input.
fn scan(input: &str) -> Result<Vec<Candidate>, String> {
    let mut out: Vec<Candidate> = Vec::new();
    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i].trim();
        if line.is_empty() || line.starts_with('#') {
            i += 1;
            continue;
        }
        if line.starts_with("-----BEGIN") && line.contains("PUBLIC KEY") {
            return Err("this input is a PEM public key block, not an OpenSSH public key. \
Convert it first (ssh-keygen -i -m PKCS8 -f key.pem) or use a PEM inspector."
                .to_string());
        }
        if line.starts_with("-----BEGIN") && line.contains("PRIVATE KEY") {
            return Err("this input is a PRIVATE key. Paste the matching public key \
(the id_*.pub line) instead — never share a private key."
                .to_string());
        }
        if line.starts_with("---- BEGIN SSH2 PUBLIC KEY") {
            let (cand, next) = scan_rfc4716(&lines, i)?;
            out.push(cand);
            i = next;
            continue;
        }
        if let Some(cand) = scan_key_line(line) {
            out.push(cand);
        }
        i += 1;
        if out.len() > MAX_KEYS {
            return Err(format!(
                "too many keys: this tool parses at most {MAX_KEYS} keys per run"
            ));
        }
    }
    Ok(out)
}

/// Parse an RFC 4716 (`---- BEGIN SSH2 PUBLIC KEY ----`) block starting at `start`.
fn scan_rfc4716(lines: &[&str], start: usize) -> Result<(Candidate, usize), String> {
    let mut comment: Option<String> = None;
    let mut body = String::new();
    let mut i = start + 1;
    let mut in_headers = true;
    let mut pending_header: Option<String> = None;
    while i < lines.len() {
        let raw = lines[i].trim();
        if raw.starts_with("---- END SSH2 PUBLIC KEY") {
            i += 1;
            break;
        }
        if let Some(prev) = pending_header.take() {
            // Continued header line (previous line ended with a backslash).
            let cont = raw.strip_suffix('\\');
            let joined = format!("{prev}{}", cont.unwrap_or(raw));
            if cont.is_some() {
                pending_header = Some(joined);
            } else if let Some(v) = joined.strip_prefix("Comment:") {
                comment = Some(v.trim().trim_matches('"').to_string());
            }
            i += 1;
            continue;
        }
        if in_headers && raw.contains(':') && !raw.is_empty() {
            if let Some(stripped) = raw.strip_suffix('\\') {
                pending_header = Some(stripped.to_string());
            } else if let Some(v) = raw.strip_prefix("Comment:") {
                comment = Some(v.trim().trim_matches('"').to_string());
            }
            i += 1;
            continue;
        }
        in_headers = false;
        body.push_str(raw);
        i += 1;
    }
    if body.is_empty() {
        return Err("RFC 4716 block contains no base64 key data between the \
'---- BEGIN SSH2 PUBLIC KEY ----' and '---- END SSH2 PUBLIC KEY ----' lines"
            .to_string());
    }
    Ok((
        Candidate {
            source_format: "rfc4716".to_string(),
            // The algorithm is only inside the blob for RFC 4716; filled in after decoding.
            algorithm: String::new(),
            body,
            comment,
            prefix: None,
            marker: None,
        },
        i,
    ))
}

/// Parse one whitespace-separated key line (openssh / authorized_keys / known_hosts).
fn scan_key_line(line: &str) -> Option<Candidate> {
    let toks: Vec<&str> = line.split_whitespace().collect();
    let mut marker = None;
    let mut start = 0usize;
    if toks.first().is_some_and(|t| t.starts_with('@')) {
        marker = Some(toks[0].to_string());
        start = 1;
    }
    let algo_idx = (start..toks.len()).find(|i| is_algo_token(toks[*i]) && *i + 1 < toks.len())?;
    let algorithm = toks[algo_idx].to_string();
    let body = toks[algo_idx + 1].to_string();
    let comment = if algo_idx + 2 < toks.len() {
        Some(toks[algo_idx + 2..].join(" "))
    } else {
        None
    };
    let prefix = if algo_idx > start {
        Some(toks[start..algo_idx].join(" "))
    } else {
        None
    };
    let source_format = match (&marker, &prefix) {
        (Some(_), _) => "known_hosts",
        (None, None) => "openssh",
        (None, Some(p)) => {
            if p.starts_with("|1|") || !looks_like_options(p) {
                "known_hosts"
            } else {
                "authorized_keys"
            }
        }
    }
    .to_string();
    Some(Candidate {
        source_format,
        algorithm,
        body,
        comment,
        prefix,
        marker,
    })
}

// ---------------------------------------------------------------------------
// Blob decoding
// ---------------------------------------------------------------------------

struct KeyFacts {
    key_type: String,
    key_size_bits: u32,
    curve: Option<String>,
    fido_application: Option<String>,
    /// The plain (non-certificate) key blob whose digest OpenSSH prints.
    plain_blob: Vec<u8>,
    certificate: Option<CertInfo>,
}

/// Read the key-material fields for `plain` out of `r`, re-encoding them into `plain_blob`.
fn read_key_material(
    plain: &str,
    r: &mut Reader,
    plain_blob: &mut Vec<u8>,
) -> Result<(String, u32, Option<String>, Option<String>), String> {
    let mut curve = None;
    let mut fido = None;
    let (key_type, bits) = match plain {
        "ssh-rsa" | "rsa-sha2-256" | "rsa-sha2-512" => {
            let e = r.bytes("RSA exponent")?;
            let n = r.bytes("RSA modulus")?;
            wire_string(plain_blob, e);
            wire_string(plain_blob, n);
            ("RSA".to_string(), mpint_bits(n))
        }
        "ssh-dss" => {
            let p = r.bytes("DSA prime p")?;
            let q = r.bytes("DSA subprime q")?;
            let g = r.bytes("DSA generator g")?;
            let y = r.bytes("DSA public value y")?;
            for f in [p, q, g, y] {
                wire_string(plain_blob, f);
            }
            ("DSA".to_string(), mpint_bits(p))
        }
        "ssh-ed25519" => {
            let pk = r.bytes("Ed25519 public key")?;
            wire_string(plain_blob, pk);
            ("Ed25519".to_string(), 256)
        }
        "ssh-ed448" => {
            let pk = r.bytes("Ed448 public key")?;
            wire_string(plain_blob, pk);
            ("Ed448".to_string(), 448)
        }
        "sk-ssh-ed25519@openssh.com" => {
            let pk = r.bytes("Ed25519 public key")?;
            let app = r.string("FIDO application")?;
            wire_string(plain_blob, pk);
            wire_string(plain_blob, app.as_bytes());
            fido = Some(app);
            ("Ed25519 (FIDO security key)".to_string(), 256)
        }
        a if a.starts_with("ecdsa-sha2-nistp") || a.starts_with("sk-ecdsa-sha2-nistp") => {
            let c = r.string("ECDSA curve name")?;
            let q = r.bytes("ECDSA public point")?;
            wire_string(plain_blob, c.as_bytes());
            wire_string(plain_blob, q);
            let bits = match c.as_str() {
                "nistp256" => 256,
                "nistp384" => 384,
                "nistp521" => 521,
                _ => 0,
            };
            curve = Some(c);
            if a.starts_with("sk-") {
                let app = r.string("FIDO application")?;
                wire_string(plain_blob, app.as_bytes());
                fido = Some(app);
                ("ECDSA (FIDO security key)".to_string(), bits)
            } else {
                ("ECDSA".to_string(), bits)
            }
        }
        other => {
            return Err(format!(
                "unsupported key algorithm '{other}': expected ssh-rsa, ssh-ed25519, ssh-dss, \
ecdsa-sha2-nistp256/384/521, an sk-* FIDO key, or any of these as a *-cert-v01@openssh.com \
certificate"
            ))
        }
    };
    Ok((key_type, bits, curve, fido))
}

/// Read the certificate envelope that follows the key material.
fn read_cert_tail(r: &mut Reader, now: i64) -> Result<CertInfo, String> {
    let serial = r.u64("certificate serial")?;
    let cert_type = match r.u32("certificate type")? {
        1 => "user",
        2 => "host",
        other => return Err(format!("unknown certificate type {other}: expected 1 (user) or 2 (host)")),
    }
    .to_string();
    let key_id = r.string("certificate key id")?;
    let principals_raw = r.bytes("certificate principals")?;
    let mut principals = Vec::new();
    let mut pr = Reader::new(principals_raw);
    while pr.remaining() > 0 {
        principals.push(pr.string("principal")?);
    }
    let valid_after = r.u64("certificate valid-after")?;
    let valid_before = r.u64("certificate valid-before")?;
    let critical_raw = r.bytes("certificate critical options")?;
    let critical_options = read_named_pairs(critical_raw, true)?;
    let ext_raw = r.bytes("certificate extensions")?;
    let extensions = read_named_pairs(ext_raw, false)?;
    let _reserved = r.bytes("certificate reserved field")?;
    let ca_blob = r.bytes("certificate signature key")?;
    let mut car = Reader::new(ca_blob);
    let ca_algorithm = car.string("CA key algorithm")?;
    let sig_raw = r.bytes("certificate signature")?;
    let mut sr = Reader::new(sig_raw);
    let signature_algorithm = sr.string("signature algorithm")?;

    let (status, days_until_expiry) = if now > 0 {
        let now_u = now as u64;
        let st = if now_u < valid_after {
            "not_yet_valid"
        } else if valid_before != FOREVER && now_u >= valid_before {
            "expired"
        } else {
            "valid"
        };
        let days = if valid_before == FOREVER {
            None
        } else {
            Some((valid_before as i64 - now) / 86_400)
        };
        (Some(st.to_string()), days)
    } else {
        (None, None)
    };

    Ok(CertInfo {
        cert_type,
        serial,
        key_id,
        principals,
        valid_after: iso8601(valid_after),
        valid_before: iso8601(valid_before),
        valid_after_unix: valid_after,
        valid_before_unix: valid_before,
        status,
        days_until_expiry,
        critical_options,
        extensions,
        ca_algorithm,
        ca_fingerprint_sha256: format!("SHA256:{}", b64_nopad(&Sha256::digest(ca_blob))),
        signature_algorithm,
    })
}

/// Decode a `(name, data)` sequence; critical options carry a string-wrapped value.
fn read_named_pairs(raw: &[u8], with_values: bool) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut r = Reader::new(raw);
    while r.remaining() > 0 {
        let name = r.string("option name")?;
        let data = r.bytes("option data")?;
        if with_values && !data.is_empty() {
            let mut dr = Reader::new(data);
            match dr.string("option value") {
                Ok(v) => out.push(format!("{name}={v}")),
                Err(_) => out.push(name),
            }
        } else {
            out.push(name);
        }
    }
    Ok(out)
}

/// Decode a base64 key blob into the facts we report.
fn decode_blob(blob: &[u8], now: i64) -> Result<(String, KeyFacts), String> {
    let mut r = Reader::new(blob);
    let embedded = r.string("key algorithm")?;
    let is_cert = embedded.ends_with(CERT_SUFFIX);
    let plain = plain_name_of(&embedded);
    let mut plain_blob = Vec::with_capacity(blob.len());
    wire_string(&mut plain_blob, plain.as_bytes());
    if is_cert {
        let _nonce = r.bytes("certificate nonce")?;
    }
    let (key_type, key_size_bits, curve, fido_application) =
        read_key_material(&plain, &mut r, &mut plain_blob)?;
    let certificate = if is_cert {
        Some(read_cert_tail(&mut r, now)?)
    } else {
        None
    };
    Ok((
        embedded,
        KeyFacts {
            key_type,
            key_size_bits,
            curve,
            fido_application,
            plain_blob,
            certificate,
        },
    ))
}

// ---------------------------------------------------------------------------
// Fingerprint matching + strength
// ---------------------------------------------------------------------------

/// Normalise a user-supplied fingerprint: drop the algorithm prefix, colons and spaces.
fn normalise_expected(s: &str) -> String {
    let t = s.trim();
    let t = t
        .strip_prefix("SHA256:")
        .or_else(|| t.strip_prefix("sha256:"))
        .or_else(|| t.strip_prefix("MD5:"))
        .or_else(|| t.strip_prefix("md5:"))
        .or_else(|| t.strip_prefix("SHA1:"))
        .or_else(|| t.strip_prefix("sha1:"))
        .unwrap_or(t);
    t.replace([':', ' '], "")
}

fn strength_of(key_type: &str, bits: u32) -> (String, Vec<String>) {
    let mut warnings = Vec::new();
    let strength = match key_type {
        t if t.starts_with("DSA") => {
            warnings.push(
                "DSA (ssh-dss) is obsolete: OpenSSH disables it by default since 7.0 and removed \
it in 9.8. Replace this key with Ed25519."
                    .to_string(),
            );
            "weak"
        }
        t if t.starts_with("RSA") => {
            if bits < 2048 {
                warnings.push(format!(
                    "{bits}-bit RSA is below the 2048-bit minimum OpenSSH accepts by default. \
Replace this key."
                ));
                "weak"
            } else if bits < 3072 {
                warnings.push(format!(
                    "{bits}-bit RSA is accepted but no longer generous; 3072 bits or Ed25519 is \
the current recommendation."
                ));
                "acceptable"
            } else {
                "strong"
            }
        }
        t if t.starts_with("ECDSA") => {
            if bits == 0 {
                warnings.push(
                    "unrecognised ECDSA curve — OpenSSH supports nistp256, nistp384 and nistp521."
                        .to_string(),
                );
                "weak"
            } else {
                "strong"
            }
        }
        _ => "strong",
    };
    (strength.to_string(), warnings)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse every OpenSSH public key in `input`.
pub fn parse(input: &str, opts: &Options) -> Result<ParseReport, String> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; this tool accepts at most {MAX_INPUT_BYTES} bytes ({} KiB) per run",
            input.len(),
            MAX_INPUT_BYTES / 1024
        ));
    }
    if input.trim().is_empty() {
        return Err("no input: paste an OpenSSH public key, e.g. \
'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAA… user@host'"
            .to_string());
    }
    let candidates = scan(input)?;
    if candidates.is_empty() {
        return Err("no SSH public key found in the input: expected a line like \
'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAA… user@host', an authorized_keys or known_hosts entry, \
or a '---- BEGIN SSH2 PUBLIC KEY ----' block"
            .to_string());
    }

    let expected = normalise_expected(&opts.expected_fingerprint);
    let want_match = !expected.is_empty();
    let mut keys: Vec<KeyReport> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for (i, c) in candidates.iter().enumerate() {
        match report_for(c, i + 1, opts, &expected, want_match) {
            Ok(k) => keys.push(k),
            Err(e) => errors.push(format!("key {}: {e}", i + 1)),
        }
    }
    if keys.is_empty() {
        return Err(errors
            .into_iter()
            .next()
            .unwrap_or_else(|| "no SSH public key could be decoded".to_string()));
    }
    // Surface per-key failures alongside the keys that did decode.
    if !errors.is_empty() {
        if let Some(last) = keys.last_mut() {
            for e in errors {
                last.warnings.push(format!("skipped an unparseable entry — {e}"));
            }
        }
    }

    let mut seen: Vec<&str> = Vec::new();
    for k in &keys {
        if !seen.contains(&k.fingerprint_sha256.as_str()) {
            seen.push(&k.fingerprint_sha256);
        }
    }
    let expected_fingerprint_matched = if want_match {
        Some(keys.iter().any(|k| k.fingerprint_match == Some(true)))
    } else {
        None
    };
    Ok(ParseReport {
        key_count: keys.len(),
        unique_fingerprints: seen.len(),
        expected_fingerprint_matched,
        keys,
    })
}

fn report_for(
    c: &Candidate,
    index: usize,
    opts: &Options,
    expected: &str,
    want_match: bool,
) -> Result<KeyReport, String> {
    let blob = b64().decode(c.body.trim()).map_err(|e| {
        format!(
            "the key body is not valid base64 ({e}). Make sure the whole 'AAAA…' field was pasted \
on one line, with no wrapping."
        )
    })?;
    let (embedded, facts) = decode_blob(&blob, opts.now)?;

    let sha256 = Sha256::digest(&facts.plain_blob);
    let md5 = Md5::digest(&facts.plain_blob);
    let sha1 = Sha1::digest(&facts.plain_blob);
    let fingerprint_sha256 = format!("SHA256:{}", b64_nopad(&sha256));
    let fingerprint_md5 = format!("MD5:{}", hex_colons(&md5, opts.uppercase_md5));
    let fingerprint_sha1 = if opts.include_sha1 {
        Some(format!("SHA1:{}", b64_nopad(&sha1)))
    } else {
        None
    };

    let algorithm = if c.algorithm.is_empty() {
        embedded.clone()
    } else {
        c.algorithm.clone()
    };
    let (strength, mut warnings) = strength_of(&facts.key_type, facts.key_size_bits);
    if !c.algorithm.is_empty() && c.algorithm != embedded {
        warnings.push(format!(
            "the line declares '{}' but the key blob contains '{embedded}' — these should match",
            c.algorithm
        ));
    }
    if let Some(cert) = &facts.certificate {
        if cert.status.as_deref() == Some("expired") {
            warnings.push(format!(
                "this certificate expired on {} and will be rejected by sshd",
                cert.valid_before
            ));
        }
        if cert.status.as_deref() == Some("not_yet_valid") {
            warnings.push(format!(
                "this certificate is not valid until {}",
                cert.valid_after
            ));
        }
        if cert.cert_type == "user" && cert.principals.is_empty() {
            warnings.push(
                "this user certificate lists no principals, so it is valid for ANY username"
                    .to_string(),
            );
        }
    }

    let fingerprint_match = if want_match {
        let sha256_hex = hex_plain(&sha256);
        let md5_hex = hex_plain(&md5);
        let sha1_hex = hex_plain(&sha1);
        let lower = expected.to_ascii_lowercase();
        Some(
            expected == b64_nopad(&sha256)
                || expected == b64_nopad(&sha1)
                || lower == sha256_hex
                || lower == md5_hex
                || lower == sha1_hex,
        )
    } else {
        None
    };

    let (authorized_keys_options, host_patterns, hashed_hosts) = match (&c.prefix, c.source_format.as_str()) {
        (Some(p), "authorized_keys") => (Some(split_options(p)), None, None),
        (Some(p), "known_hosts") => {
            let hashed = p.starts_with("|1|");
            let hosts = if hashed {
                vec![p.to_string()]
            } else {
                p.split(',').map(|h| h.trim().to_string()).filter(|h| !h.is_empty()).collect()
            };
            (None, Some(hosts), Some(hashed))
        }
        _ => (None, None, None),
    };

    Ok(KeyReport {
        index,
        source_format: c.source_format.clone(),
        algorithm,
        key_type: facts.key_type,
        is_certificate: facts.certificate.is_some(),
        key_size_bits: facts.key_size_bits,
        curve: facts.curve,
        comment: c.comment.clone().filter(|s| !s.is_empty()),
        fingerprint_sha256,
        fingerprint_md5,
        fingerprint_sha1,
        blob_bytes: blob.len(),
        authorized_keys_options,
        host_patterns,
        hashed_hosts,
        marker: c.marker.clone(),
        fido_application: facts.fido_application,
        certificate: facts.certificate,
        strength,
        warnings,
        fingerprint_match,
    })
}

/// Parse `input` and return the report as pretty-printed JSON.
pub fn run(input: &str, opts: &Options) -> Result<String, String> {
    let report = parse(input, opts)?;
    serde_json::to_string_pretty(&report).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ED: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIPc21YeL9wdmn0Bvy1dVCZH/rO/hcbVFBt5YQ/Y8+oOy alice@example.com";
    const RSA: &str = "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQCCtDYJ7yM3ARrV1+Y9sIh+8uxTFY5nS/uYvS2oj/RZnrtn+nwRMgqghyDGaHpDxXMOG4yjtn20Itt00BQng7H4H74SaT6i6lgnaZDB7JwIgGGQ7eTHxSvwnLY4T/AMPripTpjckfa1PiS3lqoTUiV8kVqkwKW5bVTD4alC77wduplX3qwUm0Np79YeqDu61HbydVJ7rUSr0vS9Yy86ONhpnGBDIDMKXFiivQpry5eZWZ/7RKLxC9qrM6Ct3UOcI+FTEIoixzc+1l0EgltlkScO7BSsiA3esefc9Mne/5MCH5gCbbAojzbOixwZlxPWihURDdMD8z7enUO78Q6evLNN build@ci";
    const EC: &str = "ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBN3/uf+t2fI/6Zf0xTF6yK8YLLiYDcO6m8h4H5vU5Qrfc8rj95WOLmYBec2w54OFTl9+9iCOgEvM/4J0i2Giudo= ops@host";
    const RSA1024: &str = "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAAAgQDekXDjgNS3ZxCX3k9Vy2bXyMehgtLFWjqnh/lDhATJjzF97/zmGaPA/4qCyYd5dzfRkwwjldgT+SSbsiPIcREcQKWAF9/jzi5OQh2jOmcaCtUpPs8yRQTBngXobCX2DZx69ZRW01iKRHqWMdKRuyaTGBC4OqQ803k5hIcAuo6KTQ== weak@old";
    const CERT: &str = "ssh-ed25519-cert-v01@openssh.com AAAAIHNzaC1lZDI1NTE5LWNlcnQtdjAxQG9wZW5zc2guY29tAAAAIBPeXO+5lpPnNa0iFwKF/NV5PDw9ngClh619XcraCsgGAAAAIPc21YeL9wdmn0Bvy1dVCZH/rO/hcbVFBt5YQ/Y8+oOyAAAAAAAAACoAAAABAAAACmFsaWNlLTIwMjYAAAARAAAABWFsaWNlAAAABHJvb3QAAAAAan3gZAAAAABsXcK6AAAAAAAAAIIAAAAVcGVybWl0LVgxMS1mb3J3YXJkaW5nAAAAAAAAABdwZXJtaXQtYWdlbnQtZm9yd2FyZGluZwAAAAAAAAAWcGVybWl0LXBvcnQtZm9yd2FyZGluZwAAAAAAAAAKcGVybWl0LXB0eQAAAAAAAAAOcGVybWl0LXVzZXItcmMAAAAAAAAAAAAAADMAAAALc3NoLWVkMjU1MTkAAAAgnc5cmnpM7DcCFztYNgzhKgc/yzpBMjvYpbszUpcgHLoAAABTAAAAC3NzaC1lZDI1NTE5AAAAQO5V2hTsTLeYPfpcm/xl62eEcN50BAKi4akEKG+Ep4PVYL+O/PS5SnPagrFTwjk/2A7vuut3UONzIdNXbXCbYAw= alice@example.com";

    fn opts() -> Options {
        Options::default()
    }

    /// Happy path: the fingerprints match `ssh-keygen -l` byte for byte.
    #[test]
    fn parses_ed25519_key_with_ssh_keygen_fingerprints() {
        let r = parse(ED, &opts()).unwrap();
        assert_eq!(r.key_count, 1);
        let k = &r.keys[0];
        assert_eq!(k.algorithm, "ssh-ed25519");
        assert_eq!(k.key_type, "Ed25519");
        assert_eq!(k.key_size_bits, 256);
        assert_eq!(k.comment.as_deref(), Some("alice@example.com"));
        assert_eq!(k.source_format, "openssh");
        assert_eq!(
            k.fingerprint_sha256,
            "SHA256:/PcooB4wsFrX/EAwN1wlE0KJbNvM1usU1KT6lCXUah4"
        );
        assert_eq!(
            k.fingerprint_md5,
            "MD5:1e:e5:90:86:13:ab:0e:5a:24:3a:30:5d:7b:53:e3:fe"
        );
        assert_eq!(k.fingerprint_sha1, None);
        assert_eq!(k.strength, "strong");
        assert!(k.warnings.is_empty());
    }

    #[test]
    fn rsa_reports_modulus_bits_and_fingerprint() {
        let r = parse(RSA, &opts()).unwrap();
        let k = &r.keys[0];
        assert_eq!(k.key_type, "RSA");
        assert_eq!(k.key_size_bits, 2048);
        assert_eq!(
            k.fingerprint_sha256,
            "SHA256:CvPu4mYra3ZtT9ct2ULG3jlE3h+QE1lIWst6/kLMCsM"
        );
        assert_eq!(
            k.fingerprint_md5,
            "MD5:09:a4:09:ec:ec:cf:68:eb:76:cd:45:f3:99:fe:9e:30"
        );
        assert_eq!(k.strength, "acceptable");
        assert!(k.warnings[0].contains("3072"));
    }

    #[test]
    fn ecdsa_reports_curve() {
        let r = parse(EC, &opts()).unwrap();
        let k = &r.keys[0];
        assert_eq!(k.key_type, "ECDSA");
        assert_eq!(k.curve.as_deref(), Some("nistp256"));
        assert_eq!(k.key_size_bits, 256);
        assert_eq!(
            k.fingerprint_sha256,
            "SHA256:SJzRNZpV82V8TRixPPsyEYQahf7okjijrSoR6s85Az8"
        );
    }

    #[test]
    fn short_rsa_key_is_flagged_weak() {
        let r = parse(RSA1024, &opts()).unwrap();
        let k = &r.keys[0];
        assert_eq!(k.key_size_bits, 1024);
        assert_eq!(k.strength, "weak");
        assert!(k.warnings[0].contains("2048-bit minimum"));
    }

    #[test]
    fn sha1_and_uppercase_md5_are_opt_in() {
        let o = Options {
            include_sha1: true,
            uppercase_md5: true,
            ..Options::default()
        };
        let k = &parse(ED, &o).unwrap().keys[0];
        assert_eq!(
            k.fingerprint_sha1.as_deref(),
            Some("SHA1:wu8NvLyw+V/NWTfudrcG9O7ImtI")
        );
        assert_eq!(
            k.fingerprint_md5,
            "MD5:1E:E5:90:86:13:AB:0E:5A:24:3A:30:5D:7B:53:E3:FE"
        );
    }

    /// A certificate fingerprints as the key it certifies (what `ssh-keygen -l` prints).
    #[test]
    fn parses_openssh_certificate() {
        // Inside the fixture's window (2026-08-13 → 2027-08-12), so status is `valid`.
        let o = Options {
            now: 1_800_000_000,
            ..Options::default()
        };
        let k = &parse(CERT, &o).unwrap().keys[0];
        assert!(k.is_certificate);
        assert_eq!(k.key_type, "Ed25519");
        assert_eq!(
            k.fingerprint_sha256,
            "SHA256:/PcooB4wsFrX/EAwN1wlE0KJbNvM1usU1KT6lCXUah4"
        );
        let c = k.certificate.as_ref().unwrap();
        assert_eq!(c.cert_type, "user");
        assert_eq!(c.serial, 42);
        assert_eq!(c.key_id, "alice-2026");
        assert_eq!(c.principals, vec!["alice", "root"]);
        assert_eq!(c.valid_after, "2026-08-13T15:19:00Z");
        assert_eq!(c.valid_before, "2027-08-12T15:20:26Z");
        assert_eq!(c.status.as_deref(), Some("valid"));
        assert_eq!(c.ca_algorithm, "ssh-ed25519");
        assert_eq!(
            c.ca_fingerprint_sha256,
            "SHA256:7qxaOU/hEpr1IN/hqSvWGWh1momIDRMOTEvZGaBgPg0"
        );
        assert_eq!(c.critical_options, Vec::<String>::new());
        assert!(c.extensions.contains(&"permit-pty".to_string()));
    }

    #[test]
    fn expired_certificate_is_flagged() {
        let o = Options {
            now: 1_900_000_000,
            ..Options::default()
        };
        let k = &parse(CERT, &o).unwrap().keys[0];
        let c = k.certificate.as_ref().unwrap();
        assert_eq!(c.status.as_deref(), Some("expired"));
        assert!(c.days_until_expiry.unwrap() < 0);
        assert!(k.warnings.iter().any(|w| w.contains("expired")));
    }

    #[test]
    fn authorized_keys_options_are_split() {
        let line = format!("command=\"/usr/bin/backup --all\",no-pty,from=\"10.0.0.0/8\" {ED}");
        let k = &parse(&line, &opts()).unwrap().keys[0];
        assert_eq!(k.source_format, "authorized_keys");
        assert_eq!(
            k.authorized_keys_options.as_ref().unwrap(),
            &vec![
                "command=\"/usr/bin/backup --all\"".to_string(),
                "no-pty".to_string(),
                "from=\"10.0.0.0/8\"".to_string()
            ]
        );
        assert_eq!(k.comment.as_deref(), Some("alice@example.com"));
    }

    #[test]
    fn known_hosts_entry_reports_host_patterns() {
        let line = format!("github.com,140.82.121.4 {}", &ED[..ED.rfind(' ').unwrap()]);
        let k = &parse(&line, &opts()).unwrap().keys[0];
        assert_eq!(k.source_format, "known_hosts");
        assert_eq!(
            k.host_patterns.as_ref().unwrap(),
            &vec!["github.com".to_string(), "140.82.121.4".to_string()]
        );
        assert_eq!(k.hashed_hosts, Some(false));
    }

    #[test]
    fn known_hosts_marker_and_hashed_host() {
        let line = format!(
            "@cert-authority |1|F1E1x0v0yyy=|abcdEFGH12345678ijkl= {}",
            &ED[..ED.rfind(' ').unwrap()]
        );
        let k = &parse(&line, &opts()).unwrap().keys[0];
        assert_eq!(k.marker.as_deref(), Some("@cert-authority"));
        assert_eq!(k.hashed_hosts, Some(true));
    }

    #[test]
    fn rfc4716_block_is_decoded() {
        let block = "---- BEGIN SSH2 PUBLIC KEY ----\n\
Comment: \"256-bit ED25519, converted from OpenSSH\"\n\
AAAAC3NzaC1lZDI1NTE5AAAAIPc21YeL9wdmn0Bvy1dVCZH/rO/hcbVFBt5YQ/Y8+oOy\n\
---- END SSH2 PUBLIC KEY ----";
        let k = &parse(block, &opts()).unwrap().keys[0];
        assert_eq!(k.source_format, "rfc4716");
        assert_eq!(k.algorithm, "ssh-ed25519");
        assert_eq!(
            k.comment.as_deref(),
            Some("256-bit ED25519, converted from OpenSSH")
        );
        assert_eq!(
            k.fingerprint_sha256,
            "SHA256:/PcooB4wsFrX/EAwN1wlE0KJbNvM1usU1KT6lCXUah4"
        );
    }

    #[test]
    fn multiple_keys_are_counted_and_deduplicated() {
        let input = format!("{ED}\n# a comment line\n{RSA}\n{ED}\n");
        let r = parse(&input, &opts()).unwrap();
        assert_eq!(r.key_count, 3);
        assert_eq!(r.unique_fingerprints, 2);
    }

    #[test]
    fn expected_fingerprint_matches_every_printed_form() {
        for want in [
            "SHA256:/PcooB4wsFrX/EAwN1wlE0KJbNvM1usU1KT6lCXUah4",
            "/PcooB4wsFrX/EAwN1wlE0KJbNvM1usU1KT6lCXUah4",
            "MD5:1e:e5:90:86:13:ab:0e:5a:24:3a:30:5d:7b:53:e3:fe",
            "1E:E5:90:86:13:AB:0E:5A:24:3A:30:5D:7B:53:E3:FE",
        ] {
            let o = Options {
                expected_fingerprint: want.to_string(),
                ..Options::default()
            };
            let r = parse(ED, &o).unwrap();
            assert_eq!(r.expected_fingerprint_matched, Some(true), "for {want}");
            assert_eq!(r.keys[0].fingerprint_match, Some(true), "for {want}");
        }
    }

    #[test]
    fn expected_fingerprint_mismatch_is_reported() {
        let o = Options {
            expected_fingerprint: "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
            ..Options::default()
        };
        let r = parse(ED, &o).unwrap();
        assert_eq!(r.expected_fingerprint_matched, Some(false));
        assert_eq!(r.keys[0].fingerprint_match, Some(false));
    }

    /// A declared algorithm that disagrees with the blob is a real-world tampering tell.
    #[test]
    fn declared_algorithm_mismatch_warns() {
        let swapped = ED.replacen("ssh-ed25519", "ssh-rsa", 1);
        let k = &parse(&swapped, &opts()).unwrap().keys[0];
        assert!(k.warnings.iter().any(|w| w.contains("declares 'ssh-rsa'")));
    }

    #[test]
    fn dsa_key_is_parsed_and_flagged_obsolete() {
        // ssh-dss blob: string "ssh-dss" + mpint p(1024-bit), q, g, y.
        let mut blob = Vec::new();
        wire_string(&mut blob, b"ssh-dss");
        let mut p = vec![0x00u8];
        p.extend(std::iter::repeat(0xAB).take(128));
        wire_string(&mut blob, &p);
        for f in [vec![0x01u8; 20], vec![0x02u8; 128], vec![0x03u8; 128]] {
            wire_string(&mut blob, &f);
        }
        let line = format!("ssh-dss {} legacy@old", b64().encode(&blob));
        let k = &parse(&line, &opts()).unwrap().keys[0];
        assert_eq!(k.key_type, "DSA");
        assert_eq!(k.key_size_bits, 1024);
        assert_eq!(k.strength, "weak");
        assert!(k.warnings[0].contains("obsolete"));
    }

    // ---- error paths -----------------------------------------------------

    #[test]
    fn empty_input_is_an_error() {
        let err = parse("   \n\n", &opts()).unwrap_err();
        assert!(err.contains("no input"), "{err}");
    }

    #[test]
    fn non_key_text_is_an_error() {
        let err = parse("hello world, not a key", &opts()).unwrap_err();
        assert!(err.contains("no SSH public key found"), "{err}");
    }

    #[test]
    fn invalid_base64_body_is_an_error() {
        let err = parse("ssh-ed25519 !!!not-base64!!! user@host", &opts()).unwrap_err();
        assert!(err.contains("not valid base64"), "{err}");
    }

    #[test]
    fn truncated_blob_is_an_error() {
        let err = parse("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIA== user@host", &opts()).unwrap_err();
        assert!(err.contains("truncated key blob"), "{err}");
    }

    #[test]
    fn private_key_input_is_rejected_with_guidance() {
        let err = parse(
            "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXk=\n-----END OPENSSH PRIVATE KEY-----",
            &opts(),
        )
        .unwrap_err();
        assert!(err.contains("PRIVATE key"), "{err}");
    }

    #[test]
    fn pem_public_key_input_points_at_the_right_tool() {
        let err = parse(
            "-----BEGIN PUBLIC KEY-----\nMIIBIjANBg==\n-----END PUBLIC KEY-----",
            &opts(),
        )
        .unwrap_err();
        assert!(err.contains("PEM public key block"), "{err}");
    }

    #[test]
    fn oversized_input_is_rejected_at_the_cap() {
        let big = "x".repeat(MAX_INPUT_BYTES + 1);
        let err = parse(&big, &opts()).unwrap_err();
        assert!(err.contains("at most"), "{err}");
        // Exactly at the cap: parsed normally (no size error).
        let padded = format!("{ED}\n{}", "#".repeat(MAX_INPUT_BYTES - ED.len() - 1));
        assert_eq!(padded.len(), MAX_INPUT_BYTES);
        assert_eq!(parse(&padded, &opts()).unwrap().key_count, 1);
    }

    #[test]
    fn too_many_keys_is_rejected() {
        let input = format!("{ED}\n").repeat(MAX_KEYS + 1);
        let err = parse(&input, &opts()).unwrap_err();
        assert!(err.contains("too many keys"), "{err}");
    }

    #[test]
    fn unparseable_entry_among_good_keys_becomes_a_warning() {
        let input = format!("{ED}\nssh-rsa !!!bad!!! nope\n");
        let r = parse(&input, &opts()).unwrap();
        assert_eq!(r.key_count, 1);
        assert!(r.keys[0]
            .warnings
            .iter()
            .any(|w| w.contains("skipped an unparseable entry")));
    }

    #[test]
    fn run_emits_pretty_json() {
        let json = run(ED, &opts()).unwrap();
        assert!(json.starts_with("{\n"));
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["keys"][0]["key_type"], "Ed25519");
        // Optional fields stay out of the output when they don't apply.
        assert!(v["keys"][0].get("certificate").is_none());
        assert!(v["keys"][0].get("fingerprint_sha1").is_none());
    }

    #[test]
    fn iso8601_handles_epoch_and_forever() {
        assert_eq!(iso8601(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso8601(1_760_000_000), "2025-10-09T08:53:20Z");
        assert_eq!(iso8601(FOREVER), "forever");
    }
}
