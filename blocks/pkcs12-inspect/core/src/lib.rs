//! pkcs12-inspect core — pure compute, shared by the chat skill block and the web page.
//!
//! Reads the OUTER structure of a PKCS#12 (.p12/.pfx) container without the password:
//! the PFX version, the MAC parameters, every `AuthenticatedSafe` → `SafeContents`
//! entry (with its PBE/PBES2 encryption parameters when password-protected), and every
//! `SafeBag` in the plaintext ones — bag type, `friendlyName`, `localKeyID`, and full
//! certificate details for cert bags. Nothing is decrypted and no secret material is
//! ever emitted: a shrouded key bag is reported by type and algorithm only.

use serde::Serialize;
use sha2::{Digest, Sha256};
use x509_parser::prelude::*;
use x509_parser::public_key::PublicKey;

/// Largest container accepted, in decoded bytes. Real .p12 files are a few KB;
/// the cap keeps a pasted blob from exhausting the wasm sandbox's memory.
pub const MAX_DER_BYTES: usize = 4 * 1024 * 1024;

/// Everything read out of one PKCS#12 container.
#[derive(Debug, Serialize)]
pub struct Pfx {
    /// PKCS#12 `version` field — 3 for every real-world file.
    pub version: u64,
    /// Decoded container size in bytes.
    pub der_bytes: usize,
    /// Integrity-MAC parameters, or `None` for a `-nomac` container.
    pub mac: Option<MacInfo>,
    /// One entry per `ContentInfo` in the `AuthenticatedSafe`.
    pub safe_contents: Vec<SafeContentsInfo>,
    pub summary: Summary,
}

/// Password-keyed integrity MAC parameters (the MAC itself cannot be verified
/// without the password, so only its parameters are reported).
#[derive(Debug, Serialize)]
pub struct MacInfo {
    pub digest_algorithm: String,
    pub digest_algorithm_oid: String,
    pub digest_length_bytes: usize,
    pub salt_length_bytes: usize,
    pub iterations: u64,
}

/// One `ContentInfo` inside the `AuthenticatedSafe`.
#[derive(Debug, Serialize)]
pub struct SafeContentsInfo {
    /// 1-based position in the `AuthenticatedSafe`.
    pub index: usize,
    /// `data` (plaintext) / `encryptedData` (password-protected) / `envelopedData` / an OID.
    pub content_type: String,
    /// True when the bags inside could be listed without a password.
    pub plaintext: bool,
    /// Encryption parameters, present only for `encryptedData`.
    pub encryption: Option<EncryptionInfo>,
    /// Bags found inside — empty for an encrypted entry.
    pub bags: Vec<BagInfo>,
    /// Why an entry could not be listed, when applicable.
    pub note: Option<String>,
}

/// Password-based encryption parameters of a protected `SafeContents`.
#[derive(Debug, Serialize)]
pub struct EncryptionInfo {
    /// e.g. `PBES2` or `pbeWithSHAAnd40BitRC2-CBC`.
    pub scheme: String,
    pub scheme_oid: String,
    /// Key-derivation function, for PBES2 (`PBKDF2`).
    pub kdf: Option<String>,
    /// PBKDF2 pseudo-random function, e.g. `hmacWithSHA256`.
    pub prf: Option<String>,
    /// Content cipher, e.g. `AES-256-CBC`.
    pub cipher: Option<String>,
    pub iterations: Option<u64>,
    pub salt_length_bytes: Option<usize>,
    pub encrypted_content_bytes: usize,
}

/// One `SafeBag`.
#[derive(Debug, Serialize)]
pub struct BagInfo {
    /// 1-based position within its `SafeContents`.
    pub index: usize,
    /// `certBag`, `keyBag`, `pkcs8ShroudedKeyBag`, `crlBag`, `secretBag`, `safeContentsBag`.
    pub bag_type: String,
    pub bag_type_oid: String,
    pub friendly_name: Option<String>,
    /// `localKeyID` as uppercase hex — the value that pairs a key bag with its cert bag.
    pub local_key_id: Option<String>,
    /// Any other bag attributes, as `name (oid)`.
    pub other_attributes: Vec<String>,
    /// Certificate details, for an X.509 cert bag.
    pub certificate: Option<CertSummary>,
    /// Extra context (shrouded key algorithm, unparsable payload, …).
    pub note: Option<String>,
}

/// Details of an X.509 certificate carried in a `certBag`.
#[derive(Debug, Serialize)]
pub struct CertSummary {
    pub subject: String,
    pub issuer: String,
    pub serial: String,
    pub not_before: String,
    pub not_after: String,
    pub self_signed: bool,
    pub is_ca: bool,
    pub public_key: String,
    pub signature_algorithm: String,
    pub fingerprint_sha256: String,
}

/// Counts, so a caller can see the shape of the container at a glance.
#[derive(Debug, Serialize)]
pub struct Summary {
    pub certificate_bags: usize,
    pub key_bags: usize,
    pub shrouded_key_bags: usize,
    pub other_bags: usize,
    pub encrypted_safe_contents: usize,
    /// True when at least one `SafeContents` is password-protected.
    pub password_required: bool,
}

/// Parse a PKCS#12 container supplied as base64 or hex.
///
/// `encoding` is `auto`, `base64` or `hex`.
pub fn inspect(data: &str, encoding: &str) -> Result<Pfx, String> {
    let der = decode_input(data, encoding)?;
    parse_pfx(&der)
}

/// Same as [`inspect`], rendered for a surface: `format` is `text` or `json`.
pub fn run(data: &str, encoding: &str, format: &str) -> Result<String, String> {
    let pfx = inspect(data, encoding)?;
    match format {
        "json" => serde_json::to_string_pretty(&pfx).map_err(|e| e.to_string()),
        "text" => Ok(render_text(&pfx)),
        other => Err(format!("unknown format '{other}': use 'text' or 'json'")),
    }
}

// ---------------------------------------------------------------- input decoding

fn decode_input(data: &str, encoding: &str) -> Result<Vec<u8>, String> {
    let trimmed = data.trim();
    if trimmed.is_empty() {
        return Err("no input: paste the .p12/.pfx file as base64 (`base64 -w0 file.p12`) or hex"
            .to_string());
    }
    let der = match encoding {
        "base64" => decode_base64(trimmed)?,
        "hex" => decode_hex(trimmed)?,
        "auto" => {
            if looks_like_hex(trimmed) {
                decode_hex(trimmed)?
            } else {
                decode_base64(trimmed)?
            }
        }
        other => {
            return Err(format!(
                "unknown encoding '{other}': use 'auto', 'base64' or 'hex'"
            ))
        }
    };
    if der.len() > MAX_DER_BYTES {
        return Err(format!(
            "container is {} bytes, over the {} byte limit",
            der.len(),
            MAX_DER_BYTES
        ));
    }
    if der.len() < 8 {
        return Err("input decoded to fewer than 8 bytes — that is not a PKCS#12 file".to_string());
    }
    if der[0] != 0x30 {
        return Err(format!(
            "decoded data does not start with a DER SEQUENCE (0x30 expected, got 0x{:02X}) — \
this is not a PKCS#12 container. A .p12/.pfx must be supplied as its raw bytes in base64 or hex, \
not as PEM text or a password.",
            der[0]
        ));
    }
    Ok(der)
}

fn looks_like_hex(s: &str) -> bool {
    let mut digits = 0usize;
    for c in s.chars() {
        match c {
            '0'..='9' | 'a'..='f' | 'A'..='F' => digits += 1,
            ' ' | '\t' | '\r' | '\n' | ':' | '-' => {}
            _ => return false,
        }
    }
    digits >= 2 && digits % 2 == 0
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let mut nibbles: Vec<u8> = Vec::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '0'..='9' => nibbles.push(c as u8 - b'0'),
            'a'..='f' => nibbles.push(c as u8 - b'a' + 10),
            'A'..='F' => nibbles.push(c as u8 - b'A' + 10),
            ' ' | '\t' | '\r' | '\n' | ':' | '-' => {}
            other => return Err(format!("invalid hex character '{other}'")),
        }
    }
    if nibbles.len() % 2 != 0 {
        return Err("hex input has an odd number of digits".to_string());
    }
    Ok(nibbles.chunks(2).map(|p| (p[0] << 4) | p[1]).collect())
}

fn decode_base64(s: &str) -> Result<Vec<u8>, String> {
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    let mut out = Vec::with_capacity(s.len() * 3 / 4 + 3);
    for c in s.chars() {
        let v = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' | '-' => 62,
            '/' | '_' => 63,
            '=' => break,
            ' ' | '\t' | '\r' | '\n' => continue,
            other => {
                return Err(format!(
                    "invalid base64 character '{other}' — paste the file as base64 (e.g. \
`base64 -w0 file.p12`) or as hex"
                ))
            }
        };
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    if out.is_empty() {
        return Err("base64 input decoded to no bytes".to_string());
    }
    Ok(out)
}

// ------------------------------------------------------------------- DER reader

struct Tlv<'a> {
    class: u8,
    constructed: bool,
    tag: u32,
    content: &'a [u8],
}

impl Tlv<'_> {
    fn is(&self, tag: u32) -> bool {
        self.class == 0 && self.tag == tag
    }
    fn is_context(&self, tag: u32) -> bool {
        self.class == 2 && self.tag == tag
    }
}

fn read_tlv<'a>(input: &'a [u8], what: &str) -> Result<(Tlv<'a>, &'a [u8]), String> {
    if input.is_empty() {
        return Err(format!("truncated DER while reading {what}"));
    }
    let id = input[0];
    let class = id >> 6;
    let constructed = id & 0x20 != 0;
    let mut tag = (id & 0x1f) as u32;
    let mut pos = 1usize;
    if tag == 0x1f {
        tag = 0;
        loop {
            if pos >= input.len() {
                return Err(format!("truncated DER tag while reading {what}"));
            }
            let b = input[pos];
            pos += 1;
            tag = tag
                .checked_mul(128)
                .and_then(|t| t.checked_add((b & 0x7f) as u32))
                .ok_or_else(|| format!("DER tag too large while reading {what}"))?;
            if b & 0x80 == 0 {
                break;
            }
        }
    }
    if pos >= input.len() {
        return Err(format!("truncated DER length while reading {what}"));
    }
    let first = input[pos];
    pos += 1;
    let len = if first & 0x80 == 0 {
        first as usize
    } else {
        let n = (first & 0x7f) as usize;
        if n == 0 {
            return Err(format!(
                "indefinite-length DER is not valid in a PKCS#12 file (while reading {what})"
            ));
        }
        if n > 4 {
            return Err(format!("DER length over 4 bytes while reading {what}"));
        }
        if pos + n > input.len() {
            return Err(format!("truncated DER length while reading {what}"));
        }
        let mut v = 0usize;
        for _ in 0..n {
            v = (v << 8) | input[pos] as usize;
            pos += 1;
        }
        v
    };
    if pos + len > input.len() {
        return Err(format!(
            "DER element claims {len} content bytes but only {} remain (while reading {what})",
            input.len() - pos
        ));
    }
    Ok((
        Tlv {
            class,
            constructed,
            tag,
            content: &input[pos..pos + len],
        },
        &input[pos + len..],
    ))
}

/// Read the single top-level element of `input`, rejecting trailing bytes.
fn read_only<'a>(input: &'a [u8], what: &str) -> Result<Tlv<'a>, String> {
    let (tlv, rest) = read_tlv(input, what)?;
    if !rest.is_empty() {
        return Err(format!("{} trailing bytes after {what}", rest.len()));
    }
    Ok(tlv)
}

/// Split a SEQUENCE/SET body into its elements.
fn read_all<'a>(mut input: &'a [u8], what: &str) -> Result<Vec<Tlv<'a>>, String> {
    let mut out = Vec::new();
    while !input.is_empty() {
        let (tlv, rest) = read_tlv(input, what)?;
        out.push(tlv);
        input = rest;
    }
    Ok(out)
}

fn expect_seq<'a>(tlv: &Tlv<'a>, what: &str) -> Result<&'a [u8], String> {
    if !(tlv.is(16) && tlv.constructed) {
        return Err(format!("expected a SEQUENCE for {what}"));
    }
    Ok(tlv.content)
}

fn read_u64(tlv: &Tlv, what: &str) -> Result<u64, String> {
    if !tlv.is(2) {
        return Err(format!("expected an INTEGER for {what}"));
    }
    let bytes: &[u8] = tlv.content;
    let bytes = bytes.strip_prefix(&[0u8]).unwrap_or(bytes);
    if bytes.len() > 8 {
        return Err(format!("{what} integer is too large"));
    }
    let mut v = 0u64;
    for b in bytes {
        v = (v << 8) | *b as u64;
    }
    Ok(v)
}

fn read_oid(tlv: &Tlv, what: &str) -> Result<String, String> {
    if !tlv.is(6) {
        return Err(format!("expected an OBJECT IDENTIFIER for {what}"));
    }
    let bytes = tlv.content;
    if bytes.is_empty() {
        return Err(format!("empty OBJECT IDENTIFIER for {what}"));
    }
    let mut out = String::new();
    let first = bytes[0] as u32;
    out.push_str(&format!("{}.{}", (first / 40).min(2), {
        if first >= 80 {
            first - 80
        } else {
            first % 40
        }
    }));
    let mut acc: u64 = 0;
    for b in &bytes[1..] {
        acc = acc
            .checked_mul(128)
            .and_then(|a| a.checked_add((b & 0x7f) as u64))
            .ok_or_else(|| format!("OID arc too large for {what}"))?;
        if b & 0x80 == 0 {
            out.push_str(&format!(".{acc}"));
            acc = 0;
        }
    }
    Ok(out)
}

/// `[0] EXPLICIT` wrapper → the single element inside it.
fn explicit_inner<'a>(tlv: &Tlv<'a>, what: &str) -> Result<Tlv<'a>, String> {
    if !tlv.is_context(0) {
        return Err(format!("expected a [0] element for {what}"));
    }
    read_only(tlv.content, what)
}

/// Concatenate a possibly-segmented OCTET STRING (constructed form).
fn octets(tlv: &Tlv, what: &str) -> Result<Vec<u8>, String> {
    if !tlv.is(4) {
        return Err(format!("expected an OCTET STRING for {what}"));
    }
    if !tlv.constructed {
        return Ok(tlv.content.to_vec());
    }
    let mut out = Vec::new();
    for part in read_all(tlv.content, what)? {
        out.extend_from_slice(&octets(&part, what)?);
    }
    Ok(out)
}

fn bmp_to_string(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks(2)
        .map(|c| ((c[0] as u16) << 8) | *c.get(1).unwrap_or(&0) as u16)
        .collect();
    String::from_utf16_lossy(&units)
}

fn hex_upper(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02X}"));
    }
    s
}

fn hex_colon(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            s.push(':');
        }
        s.push_str(&format!("{b:02X}"));
    }
    s
}

// ------------------------------------------------------------------ PKCS#12 walk

const OID_DATA: &str = "1.2.840.113549.1.7.1";
const OID_SIGNED_DATA: &str = "1.2.840.113549.1.7.2";
const OID_ENVELOPED_DATA: &str = "1.2.840.113549.1.7.3";
const OID_ENCRYPTED_DATA: &str = "1.2.840.113549.1.7.6";

fn parse_pfx(der: &[u8]) -> Result<Pfx, String> {
    let top = read_only(der, "the PFX SEQUENCE")?;
    let body = expect_seq(&top, "the PFX")?;
    let fields = read_all(body, "the PFX fields")?;
    if fields.len() < 2 {
        return Err("PFX SEQUENCE has fewer than 2 fields — not a PKCS#12 container".to_string());
    }
    let version = read_u64(&fields[0], "the PFX version")?;
    if version != 3 {
        return Err(format!(
            "unexpected PKCS#12 version {version} (every real-world .p12/.pfx is version 3)"
        ));
    }

    // authSafe: ContentInfo whose contentType MUST be `data`.
    let auth_safe_fields = read_all(
        expect_seq(&fields[1], "the authSafe ContentInfo")?,
        "the authSafe ContentInfo",
    )?;
    if auth_safe_fields.len() < 2 {
        return Err("authSafe ContentInfo has no content".to_string());
    }
    let auth_type = read_oid(&auth_safe_fields[0], "the authSafe content type")?;
    if auth_type != OID_DATA {
        return Err(format!(
            "authSafe content type is {} ({}), but PKCS#12 requires plain `data`; \
signed (public-key integrity) containers are not supported",
            content_type_name(&auth_type),
            auth_type
        ));
    }
    let auth_inner = explicit_inner(&auth_safe_fields[1], "the authSafe content")?;
    let auth_bytes = octets(&auth_inner, "the AuthenticatedSafe")?;
    let auth_safe = read_only(&auth_bytes, "the AuthenticatedSafe SEQUENCE")?;
    let entries = read_all(
        expect_seq(&auth_safe, "the AuthenticatedSafe")?,
        "the AuthenticatedSafe entries",
    )?;

    let mut safe_contents = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        safe_contents.push(parse_safe_contents_entry(i + 1, entry));
    }

    let mac = match fields.get(2) {
        Some(t) => Some(parse_mac_data(t)?),
        None => None,
    };

    let mut summary = Summary {
        certificate_bags: 0,
        key_bags: 0,
        shrouded_key_bags: 0,
        other_bags: 0,
        encrypted_safe_contents: 0,
        password_required: false,
    };
    for sc in &safe_contents {
        if !sc.plaintext {
            summary.encrypted_safe_contents += 1;
            summary.password_required = true;
        }
        for bag in &sc.bags {
            match bag.bag_type.as_str() {
                "certBag" => summary.certificate_bags += 1,
                "keyBag" => summary.key_bags += 1,
                "pkcs8ShroudedKeyBag" => {
                    summary.shrouded_key_bags += 1;
                    summary.password_required = true;
                }
                _ => summary.other_bags += 1,
            }
        }
    }

    Ok(Pfx {
        version,
        der_bytes: der.len(),
        mac,
        safe_contents,
        summary,
    })
}

fn content_type_name(oid: &str) -> &'static str {
    match oid {
        OID_DATA => "data",
        OID_SIGNED_DATA => "signedData",
        OID_ENVELOPED_DATA => "envelopedData",
        OID_ENCRYPTED_DATA => "encryptedData",
        _ => "unknown",
    }
}

/// A malformed entry becomes a noted entry rather than failing the whole file —
/// listing the rest of the container is still useful.
fn parse_safe_contents_entry(index: usize, entry: &Tlv) -> SafeContentsInfo {
    let mut info = SafeContentsInfo {
        index,
        content_type: "unknown".to_string(),
        plaintext: false,
        encryption: None,
        bags: Vec::new(),
        note: None,
    };
    let fields = match expect_seq(entry, "a SafeContents ContentInfo")
        .and_then(|b| read_all(b, "a SafeContents ContentInfo"))
    {
        Ok(f) => f,
        Err(e) => {
            info.note = Some(e);
            return info;
        }
    };
    let oid = match fields.first().map(|f| read_oid(f, "a content type")) {
        Some(Ok(o)) => o,
        Some(Err(e)) => {
            info.note = Some(e);
            return info;
        }
        None => {
            info.note = Some("empty ContentInfo".to_string());
            return info;
        }
    };
    info.content_type = format!("{} ({oid})", content_type_name(&oid));
    let content = match fields.get(1) {
        Some(c) => c,
        None => {
            info.note = Some("ContentInfo carries no content".to_string());
            return info;
        }
    };

    match oid.as_str() {
        OID_DATA => {
            info.plaintext = true;
            match explicit_inner(content, "the SafeContents content")
                .and_then(|inner| octets(&inner, "the SafeContents"))
                .and_then(|bytes| {
                    let seq = read_only(&bytes, "the SafeContents SEQUENCE")?;
                    let bags = read_all(
                        expect_seq(&seq, "the SafeContents")?,
                        "the SafeContents bags",
                    )?;
                    Ok(bags
                        .iter()
                        .enumerate()
                        .map(|(i, b)| parse_bag(i + 1, b))
                        .collect::<Vec<_>>())
                }) {
                Ok(bags) => info.bags = bags,
                Err(e) => info.note = Some(e),
            }
        }
        OID_ENCRYPTED_DATA => match parse_encrypted_data(content) {
            Ok(enc) => {
                info.encryption = Some(enc);
                info.note = Some(
                    "password-protected: bag contents are encrypted and are not listed".to_string(),
                );
            }
            Err(e) => info.note = Some(e),
        },
        OID_ENVELOPED_DATA => {
            info.note = Some(
                "public-key enveloped SafeContents — a recipient private key would be needed to \
list its bags".to_string(),
            );
        }
        _ => {
            info.note = Some(format!("unsupported SafeContents content type {oid}"));
        }
    }
    info
}

fn parse_encrypted_data(content: &Tlv) -> Result<EncryptionInfo, String> {
    let inner = explicit_inner(content, "the EncryptedData")?;
    let fields = read_all(
        expect_seq(&inner, "the EncryptedData")?,
        "the EncryptedData fields",
    )?;
    let eci = fields
        .get(1)
        .ok_or_else(|| "EncryptedData has no EncryptedContentInfo".to_string())?;
    let eci_fields = read_all(
        expect_seq(eci, "the EncryptedContentInfo")?,
        "the EncryptedContentInfo fields",
    )?;
    let alg = eci_fields
        .get(1)
        .ok_or_else(|| "EncryptedContentInfo has no algorithm".to_string())?;
    let encrypted_content_bytes = eci_fields
        .get(2)
        .map(|c| {
            // [0] IMPLICIT OCTET STRING — content bytes, possibly segmented.
            if c.constructed {
                read_all(c.content, "the encrypted content")
                    .map(|parts| parts.iter().map(|p| p.content.len()).sum())
                    .unwrap_or(c.content.len())
            } else {
                c.content.len()
            }
        })
        .unwrap_or(0);
    let mut enc = parse_pbe_algorithm(alg)?;
    enc.encrypted_content_bytes = encrypted_content_bytes;
    Ok(enc)
}

const OID_PBES2: &str = "1.2.840.113549.1.5.13";
const OID_PBKDF2: &str = "1.2.840.113549.1.5.12";

fn parse_pbe_algorithm(alg: &Tlv) -> Result<EncryptionInfo, String> {
    let fields = read_all(
        expect_seq(alg, "an AlgorithmIdentifier")?,
        "an AlgorithmIdentifier",
    )?;
    let oid = read_oid(
        fields
            .first()
            .ok_or_else(|| "empty AlgorithmIdentifier".to_string())?,
        "an algorithm OID",
    )?;
    let mut enc = EncryptionInfo {
        scheme: pbe_name(&oid).to_string(),
        scheme_oid: oid.clone(),
        kdf: None,
        prf: None,
        cipher: None,
        iterations: None,
        salt_length_bytes: None,
        encrypted_content_bytes: 0,
    };
    let params = match fields.get(1) {
        Some(p) => p,
        None => return Ok(enc),
    };
    if oid == OID_PBES2 {
        let p = read_all(expect_seq(params, "the PBES2 parameters")?, "PBES2 parameters")?;
        if let Some(kdf) = p.first() {
            let kf = read_all(expect_seq(kdf, "the PBES2 KDF")?, "the PBES2 KDF")?;
            let kdf_oid = read_oid(
                kf.first().ok_or_else(|| "empty KDF".to_string())?,
                "the KDF OID",
            )?;
            enc.kdf = Some(if kdf_oid == OID_PBKDF2 {
                "PBKDF2".to_string()
            } else {
                kdf_oid.clone()
            });
            if let Some(kp) = kf.get(1) {
                let kpf = read_all(
                    expect_seq(kp, "the PBKDF2 parameters")?,
                    "the PBKDF2 parameters",
                )?;
                if let Some(salt) = kpf.first() {
                    if salt.is(4) {
                        enc.salt_length_bytes = Some(salt.content.len());
                    }
                }
                if let Some(iter) = kpf.get(1) {
                    enc.iterations = read_u64(iter, "the PBKDF2 iteration count").ok();
                }
                // Optional keyLength then an optional PRF AlgorithmIdentifier.
                for extra in kpf.iter().skip(2) {
                    if extra.is(16) {
                        if let Ok(prf) = read_all(extra.content, "the PBKDF2 PRF") {
                            if let Some(first) = prf.first() {
                                if let Ok(prf_oid) = read_oid(first, "the PRF OID") {
                                    enc.prf = Some(digest_name(&prf_oid).to_string());
                                }
                            }
                        }
                    }
                }
                if enc.prf.is_none() {
                    // PBKDF2's default PRF when the field is absent.
                    enc.prf = Some("hmacWithSHA1 (default)".to_string());
                }
            }
        }
        if let Some(scheme) = p.get(1) {
            if let Ok(sf) = read_all(expect_seq(scheme, "the PBES2 cipher")?, "the PBES2 cipher") {
                if let Some(first) = sf.first() {
                    if let Ok(cipher_oid) = read_oid(first, "the cipher OID") {
                        enc.cipher = Some(cipher_name(&cipher_oid).to_string());
                    }
                }
            }
        }
    } else {
        // PKCS#12 / PKCS#5 v1.5 PBE: SEQUENCE { salt OCTET STRING, iterations INTEGER }
        if let Ok(p) = read_all(expect_seq(params, "the PBE parameters")?, "PBE parameters") {
            if let Some(salt) = p.first() {
                if salt.is(4) {
                    enc.salt_length_bytes = Some(salt.content.len());
                }
            }
            if let Some(iter) = p.get(1) {
                enc.iterations = read_u64(iter, "the PBE iteration count").ok();
            }
        }
        enc.cipher = Some(pbe_name(&oid).to_string());
    }
    Ok(enc)
}

fn pbe_name(oid: &str) -> &str {
    match oid {
        OID_PBES2 => "PBES2",
        "1.2.840.113549.1.12.1.1" => "pbeWithSHAAnd128BitRC4",
        "1.2.840.113549.1.12.1.2" => "pbeWithSHAAnd40BitRC4",
        "1.2.840.113549.1.12.1.3" => "pbeWithSHAAnd3-KeyTripleDES-CBC",
        "1.2.840.113549.1.12.1.4" => "pbeWithSHAAnd2-KeyTripleDES-CBC",
        "1.2.840.113549.1.12.1.5" => "pbeWithSHAAnd128BitRC2-CBC",
        "1.2.840.113549.1.12.1.6" => "pbeWithSHAAnd40BitRC2-CBC",
        "1.2.840.113549.1.5.3" => "pbeWithMD5AndDES-CBC",
        "1.2.840.113549.1.5.10" => "pbeWithSHA1AndDES-CBC",
        "1.2.840.113549.1.5.11" => "pbeWithSHA1AndRC2-CBC",
        other => other,
    }
}

fn cipher_name(oid: &str) -> &str {
    match oid {
        "2.16.840.1.101.3.4.1.2" => "AES-128-CBC",
        "2.16.840.1.101.3.4.1.22" => "AES-192-CBC",
        "2.16.840.1.101.3.4.1.42" => "AES-256-CBC",
        "2.16.840.1.101.3.4.1.6" => "AES-128-GCM",
        "2.16.840.1.101.3.4.1.46" => "AES-256-GCM",
        "1.2.840.113549.3.7" => "DES-EDE3-CBC",
        "1.3.14.3.2.7" => "DES-CBC",
        "1.2.840.113549.3.2" => "RC2-CBC",
        other => other,
    }
}

fn digest_name(oid: &str) -> &str {
    match oid {
        "1.3.14.3.2.26" => "SHA-1",
        "2.16.840.1.101.3.4.2.1" => "SHA-256",
        "2.16.840.1.101.3.4.2.2" => "SHA-384",
        "2.16.840.1.101.3.4.2.3" => "SHA-512",
        "2.16.840.1.101.3.4.2.4" => "SHA-224",
        "1.2.840.113549.2.5" => "MD5",
        "1.2.840.113549.2.7" => "hmacWithSHA1",
        "1.2.840.113549.2.8" => "hmacWithSHA224",
        "1.2.840.113549.2.9" => "hmacWithSHA256",
        "1.2.840.113549.2.10" => "hmacWithSHA384",
        "1.2.840.113549.2.11" => "hmacWithSHA512",
        other => other,
    }
}

const OID_KEY_BAG: &str = "1.2.840.113549.1.12.10.1.1";
const OID_SHROUDED_KEY_BAG: &str = "1.2.840.113549.1.12.10.1.2";
const OID_CERT_BAG: &str = "1.2.840.113549.1.12.10.1.3";
const OID_CRL_BAG: &str = "1.2.840.113549.1.12.10.1.4";
const OID_SECRET_BAG: &str = "1.2.840.113549.1.12.10.1.5";
const OID_SAFE_CONTENTS_BAG: &str = "1.2.840.113549.1.12.10.1.6";
const OID_X509_CERT: &str = "1.2.840.113549.1.9.22.1";
const OID_SDSI_CERT: &str = "1.2.840.113549.1.9.22.2";
const OID_FRIENDLY_NAME: &str = "1.2.840.113549.1.9.20";
const OID_LOCAL_KEY_ID: &str = "1.2.840.113549.1.9.21";

fn bag_type_name(oid: &str) -> &'static str {
    match oid {
        OID_KEY_BAG => "keyBag",
        OID_SHROUDED_KEY_BAG => "pkcs8ShroudedKeyBag",
        OID_CERT_BAG => "certBag",
        OID_CRL_BAG => "crlBag",
        OID_SECRET_BAG => "secretBag",
        OID_SAFE_CONTENTS_BAG => "safeContentsBag",
        _ => "unknown",
    }
}

fn attribute_name(oid: &str) -> &str {
    match oid {
        OID_FRIENDLY_NAME => "friendlyName",
        OID_LOCAL_KEY_ID => "localKeyID",
        "1.3.6.1.4.1.311.17.1" => "microsoftCSPName",
        "1.3.6.1.4.1.311.17.2" => "microsoftLocalMachineKeyset",
        "1.3.6.1.4.1.311.17.3" => "microsoftKeyProviderNameAttr",
        "2.16.840.1.113894.746875.1.1" => "oracleTrustedKeyUsage",
        other => other,
    }
}

fn parse_bag(index: usize, bag: &Tlv) -> BagInfo {
    let mut info = BagInfo {
        index,
        bag_type: "unknown".to_string(),
        bag_type_oid: String::new(),
        friendly_name: None,
        local_key_id: None,
        other_attributes: Vec::new(),
        certificate: None,
        note: None,
    };
    let fields = match expect_seq(bag, "a SafeBag").and_then(|b| read_all(b, "a SafeBag")) {
        Ok(f) => f,
        Err(e) => {
            info.note = Some(e);
            return info;
        }
    };
    let oid = match fields.first().map(|f| read_oid(f, "a bagId")) {
        Some(Ok(o)) => o,
        Some(Err(e)) => {
            info.note = Some(e);
            return info;
        }
        None => {
            info.note = Some("empty SafeBag".to_string());
            return info;
        }
    };
    info.bag_type = bag_type_name(&oid).to_string();
    info.bag_type_oid = oid.clone();

    if let Some(attrs) = fields.get(2) {
        parse_bag_attributes(attrs, &mut info);
    }

    match oid.as_str() {
        OID_CERT_BAG => match fields.get(1).ok_or_else(|| "certBag has no value".to_string()).and_then(
            |v| {
                let cb = explicit_inner(v, "the certBag value")?;
                let cf = read_all(expect_seq(&cb, "the CertBag")?, "the CertBag")?;
                let cert_id = read_oid(
                    cf.first().ok_or_else(|| "empty CertBag".to_string())?,
                    "the certId",
                )?;
                let value = cf
                    .get(1)
                    .ok_or_else(|| "CertBag has no certValue".to_string())?;
                let inner = explicit_inner(value, "the certValue")?;
                Ok((cert_id, octets(&inner, "the certValue")?))
            },
        ) {
            Ok((cert_id, der)) => {
                if cert_id == OID_X509_CERT {
                    match describe_cert(&der) {
                        Ok(c) => info.certificate = Some(c),
                        Err(e) => info.note = Some(format!("certificate could not be parsed: {e}")),
                    }
                } else if cert_id == OID_SDSI_CERT {
                    info.note = Some("SDSI certificate (not X.509) — not decoded".to_string());
                } else {
                    info.note = Some(format!("certificate type {cert_id} is not X.509"));
                }
            }
            Err(e) => info.note = Some(e),
        },
        OID_SHROUDED_KEY_BAG => {
            let described = fields
                .get(1)
                .ok_or_else(|| "shrouded key bag has no value".to_string())
                .and_then(|v| {
                    let inner = explicit_inner(v, "the EncryptedPrivateKeyInfo")?;
                    let f = read_all(
                        expect_seq(&inner, "the EncryptedPrivateKeyInfo")?,
                        "the EncryptedPrivateKeyInfo",
                    )?;
                    let alg = f
                        .first()
                        .ok_or_else(|| "EncryptedPrivateKeyInfo has no algorithm".to_string())?;
                    parse_pbe_algorithm(alg)
                });
            info.note = Some(match described {
                Ok(enc) => format!(
                    "private key encrypted with {} — the password is required to read it",
                    describe_encryption(&enc)
                ),
                Err(e) => format!("encrypted private key (algorithm unreadable: {e})"),
            });
        }
        OID_KEY_BAG => {
            info.note = Some(
                "unencrypted PKCS#8 private key — anyone with this file has the key; \
key material is never printed".to_string(),
            );
        }
        _ => {}
    }
    info
}

fn parse_bag_attributes(attrs: &Tlv, info: &mut BagInfo) {
    if !(attrs.class == 0 && attrs.tag == 17) {
        return;
    }
    let list = match read_all(attrs.content, "the bag attributes") {
        Ok(l) => l,
        Err(_) => return,
    };
    for attr in list {
        let f = match expect_seq(&attr, "a bag attribute").and_then(|b| read_all(b, "a bag attribute"))
        {
            Ok(f) => f,
            Err(_) => continue,
        };
        let oid = match f.first().map(|t| read_oid(t, "an attribute OID")) {
            Some(Ok(o)) => o,
            _ => continue,
        };
        let values = f
            .get(1)
            .and_then(|v| read_all(v.content, "attribute values").ok())
            .unwrap_or_default();
        match oid.as_str() {
            OID_FRIENDLY_NAME => {
                if let Some(v) = values.first() {
                    // BMPString (tag 30) in practice; accept UTF8String too.
                    info.friendly_name = Some(if v.tag == 30 {
                        bmp_to_string(v.content)
                    } else {
                        String::from_utf8_lossy(v.content).to_string()
                    });
                }
            }
            OID_LOCAL_KEY_ID => {
                if let Some(v) = values.first() {
                    info.local_key_id = Some(hex_upper(v.content));
                }
            }
            other => info
                .other_attributes
                .push(format!("{} ({})", attribute_name(other), other)),
        }
    }
}

fn describe_cert(der: &[u8]) -> Result<CertSummary, String> {
    let (_, cert) =
        X509Certificate::from_der(der).map_err(|e| format!("not a valid X.509 certificate: {e}"))?;
    let validity = cert.validity();
    let is_ca = cert.basic_constraints().ok().flatten().map(|bc| bc.value.ca) == Some(true);
    let public_key = describe_spki(cert.public_key());
    let mut sha256 = Sha256::new();
    sha256.update(der);
    Ok(CertSummary {
        subject: cert.subject().to_string(),
        issuer: cert.issuer().to_string(),
        serial: cert.raw_serial_as_string(),
        not_before: validity.not_before.to_string(),
        not_after: validity.not_after.to_string(),
        self_signed: cert.subject() == cert.issuer(),
        is_ca,
        public_key,
        signature_algorithm: sig_alg_name(&cert.signature_algorithm.algorithm.to_id_string()),
        fingerprint_sha256: hex_colon(&sha256.finalize()),
    })
}

fn describe_spki(spki: &SubjectPublicKeyInfo) -> String {
    match spki.parsed() {
        Ok(PublicKey::RSA(rsa)) => format!("RSA {} bit", rsa.key_size()),
        Ok(PublicKey::EC(ec)) => format!("EC {} bit", ec.key_size()),
        Ok(PublicKey::DSA(_)) => "DSA".to_string(),
        Ok(PublicKey::GostR3410(_)) | Ok(PublicKey::GostR3410_2012(_)) => "GOST R 34.10".to_string(),
        Ok(PublicKey::Unknown(_)) | Err(_) => {
            sig_alg_name(&spki.algorithm.algorithm.to_id_string())
        }
    }
}

fn sig_alg_name(oid: &str) -> String {
    match oid {
        "1.2.840.113549.1.1.1" => "RSA",
        "1.2.840.113549.1.1.5" => "SHA-1 with RSA",
        "1.2.840.113549.1.1.11" => "SHA-256 with RSA",
        "1.2.840.113549.1.1.12" => "SHA-384 with RSA",
        "1.2.840.113549.1.1.13" => "SHA-512 with RSA",
        "1.2.840.113549.1.1.10" => "RSASSA-PSS",
        "1.2.840.10045.2.1" => "EC",
        "1.2.840.10045.4.3.2" => "ECDSA with SHA-256",
        "1.2.840.10045.4.3.3" => "ECDSA with SHA-384",
        "1.2.840.10045.4.3.4" => "ECDSA with SHA-512",
        "1.3.101.112" => "Ed25519",
        _ => return oid.to_string(),
    }
    .to_string()
}

fn parse_mac_data(tlv: &Tlv) -> Result<MacInfo, String> {
    let fields = read_all(expect_seq(tlv, "the MacData")?, "the MacData")?;
    let digest_info = fields
        .first()
        .ok_or_else(|| "MacData has no DigestInfo".to_string())?;
    let di = read_all(expect_seq(digest_info, "the DigestInfo")?, "the DigestInfo")?;
    let alg = read_all(
        expect_seq(
            di.first()
                .ok_or_else(|| "DigestInfo has no algorithm".to_string())?,
            "the MAC digest algorithm",
        )?,
        "the MAC digest algorithm",
    )?;
    let alg_oid = read_oid(
        alg.first()
            .ok_or_else(|| "empty MAC digest algorithm".to_string())?,
        "the MAC digest OID",
    )?;
    let digest_length_bytes = di.get(1).map(|d| d.content.len()).unwrap_or(0);
    let salt_length_bytes = fields
        .get(1)
        .filter(|s| s.is(4))
        .map(|s| s.content.len())
        .unwrap_or(0);
    // `iterations` is OPTIONAL with DEFAULT 1.
    let iterations = match fields.get(2) {
        Some(t) => read_u64(t, "the MAC iteration count")?,
        None => 1,
    };
    Ok(MacInfo {
        digest_algorithm: digest_name(&alg_oid).to_string(),
        digest_algorithm_oid: alg_oid,
        digest_length_bytes,
        salt_length_bytes,
        iterations,
    })
}

// ------------------------------------------------------------------ text report

fn describe_encryption(enc: &EncryptionInfo) -> String {
    let mut parts = vec![enc.scheme.clone()];
    if let Some(kdf) = &enc.kdf {
        parts.push(kdf.clone());
    }
    if let Some(cipher) = &enc.cipher {
        if *cipher != enc.scheme {
            parts.push(cipher.clone());
        }
    }
    if let Some(iter) = enc.iterations {
        parts.push(format!("{iter} iterations"));
    }
    if let Some(prf) = &enc.prf {
        parts.push(format!("PRF {prf}"));
    }
    parts.join(", ")
}

fn render_text(pfx: &Pfx) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "PKCS#12 container: version {}, {} bytes\n",
        pfx.version, pfx.der_bytes
    ));
    match &pfx.mac {
        Some(m) => out.push_str(&format!(
            "Integrity MAC: {}, {} iterations (MAC {} bytes, salt {} bytes) — not verified (needs the password)\n",
            m.digest_algorithm, m.iterations, m.digest_length_bytes, m.salt_length_bytes
        )),
        None => out.push_str("Integrity MAC: none (container has no MacData)\n"),
    }
    let s = &pfx.summary;
    out.push_str(&format!(
        "Contents: {} certificate bag(s), {} unencrypted key bag(s), {} encrypted key bag(s), {} other bag(s), {} encrypted SafeContents\n",
        s.certificate_bags, s.key_bags, s.shrouded_key_bags, s.other_bags, s.encrypted_safe_contents
    ));
    out.push_str(&format!(
        "Password required to extract: {}\n",
        if s.password_required { "yes" } else { "no" }
    ));

    for sc in &pfx.safe_contents {
        out.push_str(&format!(
            "\nSafeContents {}: {}\n",
            sc.index, sc.content_type
        ));
        if let Some(enc) = &sc.encryption {
            out.push_str(&format!("  Encryption: {}\n", describe_encryption(enc)));
            if let Some(salt) = enc.salt_length_bytes {
                out.push_str(&format!("  Salt: {salt} bytes\n"));
            }
            out.push_str(&format!(
                "  Encrypted payload: {} bytes\n",
                enc.encrypted_content_bytes
            ));
        }
        if let Some(note) = &sc.note {
            out.push_str(&format!("  Note: {note}\n"));
        }
        for bag in &sc.bags {
            out.push_str(&format!(
                "  Bag {}: {} ({})\n",
                bag.index, bag.bag_type, bag.bag_type_oid
            ));
            if let Some(name) = &bag.friendly_name {
                out.push_str(&format!("    friendlyName: {name}\n"));
            }
            if let Some(id) = &bag.local_key_id {
                out.push_str(&format!("    localKeyID: {id}\n"));
            }
            for attr in &bag.other_attributes {
                out.push_str(&format!("    attribute: {attr}\n"));
            }
            if let Some(c) = &bag.certificate {
                out.push_str(&format!("    subject: {}\n", c.subject));
                out.push_str(&format!("    issuer: {}\n", c.issuer));
                out.push_str(&format!("    serial: {}\n", c.serial));
                out.push_str(&format!("    validity: {} .. {}\n", c.not_before, c.not_after));
                out.push_str(&format!(
                    "    self-signed: {}, CA: {}\n",
                    c.self_signed, c.is_ca
                ));
                out.push_str(&format!("    public key: {}\n", c.public_key));
                out.push_str(&format!("    signature: {}\n", c.signature_algorithm));
                out.push_str(&format!("    SHA-256: {}\n", c.fingerprint_sha256));
            }
            if let Some(note) = &bag.note {
                out.push_str(&format!("    note: {note}\n"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: &str = include_str!("../tests/fixtures/plain-bags.p12.b64");
    const AES: &str = include_str!("../tests/fixtures/default-aes.p12.b64");

    #[test]
    fn lists_plaintext_cert_and_key_bags() {
        let pfx = inspect(PLAIN, "auto").expect("plaintext container parses");
        assert_eq!(pfx.version, 3);
        assert_eq!(pfx.summary.certificate_bags, 1);
        assert_eq!(pfx.summary.key_bags, 1);
        assert_eq!(pfx.summary.encrypted_safe_contents, 0);
        assert!(!pfx.summary.password_required);
        assert_eq!(pfx.safe_contents.len(), 2);

        let cert_bag = &pfx.safe_contents[0].bags[0];
        assert_eq!(cert_bag.bag_type, "certBag");
        assert_eq!(cert_bag.friendly_name.as_deref(), Some("Plain Bag Demo"));
        let cert = cert_bag.certificate.as_ref().expect("certificate decoded");
        assert!(cert.subject.contains("CN=example.test"), "{}", cert.subject);
        assert!(cert.self_signed);
        assert_eq!(cert.public_key, "RSA 2048 bit");
        assert_eq!(cert.fingerprint_sha256.len(), 32 * 3 - 1);

        let key_bag = &pfx.safe_contents[1].bags[0];
        assert_eq!(key_bag.bag_type, "keyBag");
        // Both bags carry the same localKeyID, which is how they pair up.
        assert!(key_bag.local_key_id.is_some());
        assert_eq!(key_bag.local_key_id, cert_bag.local_key_id);

        let mac = pfx.mac.as_ref().expect("MacData present");
        assert_eq!(mac.digest_algorithm, "SHA-256");
        assert_eq!(mac.iterations, 2048);
    }

    #[test]
    fn reports_pbes2_parameters_without_the_password() {
        let pfx = inspect(AES, "base64").expect("encrypted container parses");
        assert_eq!(pfx.summary.encrypted_safe_contents, 1);
        assert!(pfx.summary.password_required);

        let enc = pfx.safe_contents[0]
            .encryption
            .as_ref()
            .expect("encrypted SafeContents reports its algorithm");
        assert_eq!(enc.scheme, "PBES2");
        assert_eq!(enc.kdf.as_deref(), Some("PBKDF2"));
        assert_eq!(enc.cipher.as_deref(), Some("AES-256-CBC"));
        assert_eq!(enc.iterations, Some(2048));
        assert_eq!(enc.prf.as_deref(), Some("hmacWithSHA256"));
        assert!(pfx.safe_contents[0].bags.is_empty());

        // The shrouded key bag is listed, by algorithm only.
        let key_bag = &pfx.safe_contents[1].bags[0];
        assert_eq!(key_bag.bag_type, "pkcs8ShroudedKeyBag");
        let note = key_bag.note.as_deref().unwrap_or("");
        assert!(note.contains("AES-256-CBC"), "{note}");
    }

    #[test]
    fn text_report_and_json_render() {
        let text = run(PLAIN, "auto", "text").unwrap();
        assert!(text.starts_with("PKCS#12 container: version 3,"));
        assert!(text.contains("friendlyName: Plain Bag Demo"));
        let json = run(PLAIN, "auto", "json").unwrap();
        assert!(json.contains("\"bag_type\": \"certBag\""));
    }

    #[test]
    fn decodes_an_ec_certificate_bag() {
        const EC: &str = include_str!("../tests/fixtures/ec-plain.p12.b64");
        let pfx = inspect(EC, "auto").expect("EC container parses");
        let bag = &pfx.safe_contents[0].bags[0];
        assert_eq!(bag.friendly_name.as_deref(), Some("EC Sample"));
        let cert = bag.certificate.as_ref().expect("certificate decoded");
        assert_eq!(cert.public_key, "EC 256 bit");
        assert_eq!(cert.signature_algorithm, "ECDSA with SHA-256");
        assert!(cert.is_ca);
    }

    #[test]
    fn hex_input_is_accepted() {
        let der = decode_base64(PLAIN.trim()).unwrap();
        let hex = hex_colon(&der);
        let pfx = inspect(&hex, "auto").expect("colon-separated hex parses");
        assert_eq!(pfx.summary.certificate_bags, 1);
    }

    #[test]
    fn rejects_non_pkcs12_input() {
        let err = inspect("aGVsbG8gd29ybGQgdGhpcyBpcyBub3QgYSBwMTI=", "base64").unwrap_err();
        assert!(err.contains("not a PKCS#12 container"), "{err}");
    }

    #[test]
    fn rejects_empty_and_unknown_options() {
        assert!(inspect("   ", "auto").unwrap_err().contains("no input"));
        assert!(inspect(PLAIN, "utf8").unwrap_err().contains("unknown encoding"));
        assert!(run(PLAIN, "auto", "yaml").unwrap_err().contains("unknown format"));
    }

    #[test]
    fn rejects_truncated_container() {
        let der = decode_base64(PLAIN.trim()).unwrap();
        let half = hex_upper(&der[..der.len() / 2]);
        let err = inspect(&half, "hex").unwrap_err();
        assert!(err.contains("content bytes but only"), "{err}");
    }
}
