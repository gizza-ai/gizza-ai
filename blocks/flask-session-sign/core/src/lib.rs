//! flask-session-sign core — build a VALID Flask session cookie from a JSON
//! payload and a `SECRET_KEY`, replicating `itsdangerous`'s key derivation,
//! HMAC, and base64url encoding exactly. No wafer/wasm-bindgen deps; pure-Rust
//! (base64url + zlib via flate2 + hmac/sha1/sha2), so it runs on every backend.
//!
//! # What Flask actually does
//!
//! Flask's default session is written by `flask.sessions.SecureCookieSessionInterface`,
//! which builds an `itsdangerous.URLSafeTimedSerializer` configured with:
//!   - `salt = "cookie-session"`,
//!   - `serializer = TaggedJSONSerializer` (compact separators, sorted keys,
//!     `ensure_ascii=True`),
//!   - `signer_kwargs = {key_derivation: "hmac", digest_method: sha1}`.
//!
//! `dumps(obj)` then produces `<payload>.<timestamp>.<signature>`:
//!   1. **payload** — the session JSON, zlib-compressed *only if* that saves more
//!      than one byte, base64url-encoded without padding. A compressed payload is
//!      marked by prefixing the whole segment with a literal `.`.
//!   2. **timestamp** — `base64url(int_to_bytes(now))`, where `int_to_bytes` is a
//!      minimal-length big-endian encoding (no leading zero bytes). itsdangerous
//!      >= 1.0 signs the **full Unix timestamp**; < 1.0 signed an offset from
//!      2011-01-01 (Unix 1293840000) — that is the `legacy_epoch` mode here.
//!   3. **signature** — `base64url(HMAC(derived_key, "<payload>.<timestamp>"))`,
//!      where `derived_key` comes from the secret + salt via `key_derivation`.
//!
//! Everything is deterministic: the same payload, secret, and timestamp always
//! produce the same cookie, which is what makes this testable (and what lets a
//! user re-sign a cookie and string-compare it against one they already have).

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};
use std::io::Write;

/// itsdangerous < 1.0 epoch: 2011-01-01T00:00:00Z as seconds since the Unix epoch.
pub const ITSDANGEROUS_LEGACY_EPOCH: i64 = 1_293_840_000;

/// Flask's `SecureCookieSessionInterface.salt`.
pub const FLASK_SALT: &str = "cookie-session";

/// Largest payload we accept, in bytes of input JSON text. Well above any sane
/// session (browsers cap a whole cookie at ~4 KB) but small enough that a typo'd
/// paste fails fast instead of allocating.
pub const MAX_PAYLOAD_BYTES: usize = 65_536;

/// Practical per-cookie byte budget browsers enforce on the whole `name=value`
/// pair. RFC 6265 asks for at least 4096; every major browser uses exactly that.
pub const BROWSER_COOKIE_LIMIT: usize = 4096;

/// The keys `flask.json.tag.TaggedJSONSerializer` reserves for its type tags. A
/// one-key object using any of them must be escaped, or Flask would decode it
/// back as a tuple/bytes/datetime instead of a dict.
const RESERVED_TAGS: [&str; 6] = [" t", " b", " m", " u", " d", " di"];

/// Digest used for both key derivation and the signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DigestMethod {
    /// Flask's default (`hashlib.sha1`).
    Sha1,
    Sha256,
    Sha512,
}

impl DigestMethod {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s
            .trim()
            .to_ascii_lowercase()
            .replace(['-', '_'], "")
            .as_str()
        {
            "" | "sha1" => Ok(Self::Sha1),
            "sha256" => Ok(Self::Sha256),
            "sha512" => Ok(Self::Sha512),
            other => Err(format!(
                "unknown digest '{other}': expected sha1, sha256, or sha512"
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
            Self::Sha512 => "sha512",
        }
    }

    /// Plain (unkeyed) digest of `data` — used by the `concat` derivations.
    fn digest(self, data: &[u8]) -> Vec<u8> {
        match self {
            Self::Sha1 => Sha1::digest(data).to_vec(),
            Self::Sha256 => Sha256::digest(data).to_vec(),
            Self::Sha512 => Sha512::digest(data).to_vec(),
        }
    }

    /// Keyed HMAC of `data` — used by the `hmac` derivation and the signature.
    fn hmac(self, key: &[u8], data: &[u8]) -> Vec<u8> {
        match self {
            Self::Sha1 => {
                let mut m = Hmac::<Sha1>::new_from_slice(key).expect("hmac accepts any key length");
                m.update(data);
                m.finalize().into_bytes().to_vec()
            }
            Self::Sha256 => {
                let mut m =
                    Hmac::<Sha256>::new_from_slice(key).expect("hmac accepts any key length");
                m.update(data);
                m.finalize().into_bytes().to_vec()
            }
            Self::Sha512 => {
                let mut m =
                    Hmac::<Sha512>::new_from_slice(key).expect("hmac accepts any key length");
                m.update(data);
                m.finalize().into_bytes().to_vec()
            }
        }
    }
}

/// How the signing key is derived from the secret key + salt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyDerivation {
    /// `HMAC(secret, salt)` — what Flask configures.
    Hmac,
    /// `digest(salt + b"signer" + secret)` — itsdangerous's own default.
    DjangoConcat,
    /// `digest(salt + secret)`.
    Concat,
    /// The secret verbatim; the salt is ignored.
    None,
}

impl KeyDerivation {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "" | "hmac" => Ok(Self::Hmac),
            "django-concat" => Ok(Self::DjangoConcat),
            "concat" => Ok(Self::Concat),
            "none" => Ok(Self::None),
            other => Err(format!(
                "unknown key_derivation '{other}': expected hmac, django-concat, concat, or none"
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Hmac => "hmac",
            Self::DjangoConcat => "django-concat",
            Self::Concat => "concat",
            Self::None => "none",
        }
    }

    fn derive(self, digest: DigestMethod, secret: &[u8], salt: &[u8]) -> Vec<u8> {
        match self {
            Self::Hmac => digest.hmac(secret, salt),
            Self::DjangoConcat => {
                let mut buf = salt.to_vec();
                buf.extend_from_slice(b"signer");
                buf.extend_from_slice(secret);
                digest.digest(&buf)
            }
            Self::Concat => {
                let mut buf = salt.to_vec();
                buf.extend_from_slice(secret);
                digest.digest(&buf)
            }
            Self::None => secret.to_vec(),
        }
    }
}

/// How the `secret` text is turned into key bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretEncoding {
    /// The literal characters, UTF-8 encoded (a plain `SECRET_KEY = "..."`).
    Utf8,
    /// Hex digits (a key stored as `bytes.fromhex(...)` / `os.urandom(32).hex()`).
    Hex,
    /// Standard or URL-safe base64, padded or not.
    Base64,
}

impl SecretEncoding {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s
            .trim()
            .to_ascii_lowercase()
            .replace(['-', '_'], "")
            .as_str()
        {
            "" | "utf8" | "text" => Ok(Self::Utf8),
            "hex" => Ok(Self::Hex),
            "base64" => Ok(Self::Base64),
            other => Err(format!(
                "unknown secret_encoding '{other}': expected utf8, hex, or base64"
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Utf8 => "utf8",
            Self::Hex => "hex",
            Self::Base64 => "base64",
        }
    }

    fn decode(self, secret: &str) -> Result<Vec<u8>, String> {
        match self {
            Self::Utf8 => Ok(secret.as_bytes().to_vec()),
            Self::Hex => {
                let cleaned: String = secret
                    .chars()
                    .filter(|c| !c.is_ascii_whitespace())
                    .collect();
                if cleaned.len() % 2 != 0 {
                    return Err(format!(
                        "secret_encoding=hex expects an even number of hex digits, got {}",
                        cleaned.len()
                    ));
                }
                (0..cleaned.len())
                    .step_by(2)
                    .map(|i| {
                        u8::from_str_radix(&cleaned[i..i + 2], 16).map_err(|_| {
                            format!(
                                "secret_encoding=hex: '{}' at offset {i} is not a hex byte",
                                &cleaned[i..i + 2]
                            )
                        })
                    })
                    .collect()
            }
            Self::Base64 => {
                let cleaned: String = secret
                    .chars()
                    .filter(|c| !c.is_ascii_whitespace())
                    .collect();
                let normalized = cleaned.replace('-', "+").replace('_', "/");
                let unpadded = normalized.trim_end_matches('=');
                URL_SAFE_NO_PAD
                    .decode(unpadded.replace('+', "-").replace('/', "_").as_bytes())
                    .map_err(|e| format!("secret_encoding=base64: not valid base64 ({e})"))
            }
        }
    }
}

/// Whether to zlib-compress the serialized payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressMode {
    /// itsdangerous's rule: compress only when it saves more than one byte.
    Auto,
    /// Always compress, even when it makes the cookie bigger.
    Always,
    /// Never compress.
    Never,
}

impl CompressMode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Self::Auto),
            "always" => Ok(Self::Always),
            "never" => Ok(Self::Never),
            other => Err(format!(
                "unknown compress '{other}': expected auto, always, or never"
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Always => "always",
            Self::Never => "never",
        }
    }
}

/// Everything the signer needs. Built by the surface wrappers from their params.
#[derive(Clone, Debug)]
pub struct SignOptions {
    pub secret: String,
    pub salt: String,
    pub secret_encoding: SecretEncoding,
    pub digest: DigestMethod,
    pub key_derivation: KeyDerivation,
    /// Unix seconds to sign; `0` means "use `now_unix`".
    pub timestamp: i64,
    /// Clock reading supplied by the calling surface (each has its own clock).
    pub now_unix: i64,
    /// Sign an offset from 2011-01-01 instead of the full Unix time (itsdangerous < 1.0).
    pub legacy_epoch: bool,
    pub compress: CompressMode,
    pub cookie_name: String,
}

impl Default for SignOptions {
    fn default() -> Self {
        Self {
            secret: String::new(),
            salt: FLASK_SALT.to_string(),
            secret_encoding: SecretEncoding::Utf8,
            digest: DigestMethod::Sha1,
            key_derivation: KeyDerivation::Hmac,
            timestamp: 0,
            now_unix: 0,
            legacy_epoch: false,
            compress: CompressMode::Auto,
            cookie_name: "session".to_string(),
        }
    }
}

/// The signed cookie plus every intermediate value, so a user can check each
/// stage against their own Flask app instead of trusting one opaque string.
#[derive(Serialize, Debug)]
pub struct Signed {
    /// The full signed cookie value: `<payload>.<timestamp>.<signature>`.
    pub cookie: String,
    /// A ready-to-send response header for the cookie.
    pub set_cookie_header: String,
    /// The exact JSON text Flask would serialize the session to (sorted keys,
    /// compact separators, non-ASCII escaped) — what actually gets signed.
    pub serialized_payload: String,
    /// Segment 1: base64url of the (optionally zlib-compressed) payload.
    pub payload_segment: String,
    /// Segment 2: base64url of the minimal big-endian timestamp.
    pub timestamp_segment: String,
    /// Segment 3: base64url of the HMAC over `payload.timestamp`.
    pub signature_segment: String,
    /// Whether the payload was zlib-compressed (segment 1 then starts with `.`).
    pub compressed: bool,
    /// The signed timestamp as Unix seconds.
    pub timestamp: i64,
    /// The signed timestamp as an RFC-3339 / ISO-8601 UTC string.
    pub timestamp_iso: String,
    /// The derived signing key, hex-encoded — cross-check it against your app.
    pub derived_key_hex: String,
    /// Byte length of `<cookie_name>=<cookie>`, the thing browsers size-limit.
    pub cookie_bytes: usize,
    /// Non-fatal notes (e.g. the cookie exceeding the ~4 KB browser limit).
    pub warnings: Vec<String>,
}

fn b64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// itsdangerous `int_to_bytes`: minimal-length big-endian, no leading zeros.
/// Zero encodes as the empty byte string (and therefore an empty segment).
fn int_to_bytes(mut n: i64) -> Vec<u8> {
    if n == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    while n > 0 {
        out.push((n & 0xFF) as u8);
        n >>= 8;
    }
    out.reverse();
    out
}

/// A `serde_json` formatter that reproduces Python's `json.dumps(ensure_ascii=True)`:
/// every non-ASCII scalar becomes a `\uXXXX` escape (surrogate pair when needed).
/// serde_json still handles quotes/backslashes/control characters itself, and its
/// short escapes (`\n`, `\t`, ``, …) already match Python's.
struct EnsureAscii;

impl serde_json::ser::Formatter for EnsureAscii {
    fn write_string_fragment<W>(&mut self, writer: &mut W, fragment: &str) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        let mut ascii_start = 0usize;
        for (idx, ch) in fragment.char_indices() {
            if ch.is_ascii() {
                continue;
            }
            writer.write_all(fragment[ascii_start..idx].as_bytes())?;
            let mut buf = [0u16; 2];
            for unit in ch.encode_utf16(&mut buf) {
                write!(writer, "\\u{unit:04x}")?;
            }
            ascii_start = idx + ch.len_utf8();
        }
        writer.write_all(fragment[ascii_start..].as_bytes())
    }
}

/// Apply Flask's `TagDict` escape. `TaggedJSONSerializer` reserves a handful of
/// one-key object shapes for its own types, so a *real* one-key dict whose key is
/// one of them is rewritten as `{" di": {"<key>__": <value>}}` on the way out.
/// Everything else in plain JSON round-trips untouched.
fn tag_json(value: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            let escaped = map.len() == 1
                && map
                    .keys()
                    .next()
                    .is_some_and(|k| RESERVED_TAGS.contains(&k.as_str()));
            if escaped {
                let (key, inner) = map.iter().next().expect("len == 1");
                let mut wrapper = serde_json::Map::new();
                wrapper.insert(format!("{key}__"), tag_json(inner));
                let mut outer = serde_json::Map::new();
                outer.insert(" di".to_string(), Value::Object(wrapper));
                Value::Object(outer)
            } else {
                Value::Object(map.iter().map(|(k, v)| (k.clone(), tag_json(v))).collect())
            }
        }
        Value::Array(items) => Value::Array(items.iter().map(tag_json).collect()),
        other => other.clone(),
    }
}

/// Serialize `payload` exactly as Flask's session serializer would: tagged,
/// sorted keys, `(",", ":")` separators, `ensure_ascii=True`.
///
/// Key ordering comes for free — `serde_json::Map` is a `BTreeMap`, so keys are
/// emitted in byte order, and UTF-8 byte order equals Unicode code-point order,
/// which is what Python's `sort_keys=True` uses.
pub fn serialize_payload(payload: &str) -> Result<String, String> {
    let trimmed = payload.trim();
    if trimmed.is_empty() {
        return Err(
            "empty payload: paste the session data as a JSON object, e.g. {\"logged_in\":true}"
                .into(),
        );
    }
    if trimmed.len() > MAX_PAYLOAD_BYTES {
        return Err(format!(
            "payload is {} bytes, over the {MAX_PAYLOAD_BYTES}-byte limit",
            trimmed.len()
        ));
    }
    let value: serde_json::Value = serde_json::from_str(trimmed).map_err(|e| {
        format!("payload is not valid JSON: {e}. Use JSON spelling — true/false/null, not Python's True/False/None, and double-quoted keys")
    })?;
    if !value.is_object() {
        return Err(format!(
            "payload must be a JSON object (Flask sessions are dicts), got {}",
            json_kind(&value)
        ));
    }
    let tagged = tag_json(&value);
    let mut out = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut out, EnsureAscii);
    tagged
        .serialize(&mut ser)
        .map_err(|e| format!("could not serialize payload: {e}"))?;
    String::from_utf8(out).map_err(|e| format!("serialized payload is not UTF-8: {e}"))
}

fn json_kind(v: &serde_json::Value) -> &'static str {
    use serde_json::Value;
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// zlib-compress with Python's `zlib.compress` default level (6).
fn zlib_compress(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(data)
        .map_err(|e| format!("zlib compression failed: {e}"))?;
    enc.finish()
        .map_err(|e| format!("zlib compression failed: {e}"))
}

/// Build the signed Flask session cookie.
pub fn sign(payload: &str, opts: &SignOptions) -> Result<Signed, String> {
    if opts.secret.is_empty() {
        return Err("empty secret: pass your Flask app's SECRET_KEY".into());
    }
    let secret_bytes = opts.secret_encoding.decode(&opts.secret)?;
    if secret_bytes.is_empty() {
        return Err(format!(
            "secret decoded to 0 bytes with secret_encoding={}",
            opts.secret_encoding.name()
        ));
    }
    let cookie_name = if opts.cookie_name.trim().is_empty() {
        "session"
    } else {
        opts.cookie_name.trim()
    };
    if let Some(bad) = cookie_name
        .chars()
        .find(|c| !c.is_ascii_alphanumeric() && !matches!(c, '_' | '-' | '.'))
    {
        return Err(format!(
            "cookie_name may only contain letters, digits, '_', '-', or '.', got '{bad}'"
        ));
    }

    // 1. Serialize + optionally compress the payload.
    let serialized = serialize_payload(payload)?;
    let raw = serialized.as_bytes();
    let compressed_bytes = zlib_compress(raw)?;
    let compressed = match opts.compress {
        CompressMode::Never => false,
        CompressMode::Always => true,
        // itsdangerous only keeps the compressed form if it saves >1 byte,
        // because the marker '.' costs one.
        CompressMode::Auto => compressed_bytes.len() < raw.len().saturating_sub(1),
    };
    let payload_segment = if compressed {
        format!(".{}", b64url(&compressed_bytes))
    } else {
        b64url(raw)
    };

    // 2. Timestamp segment.
    let unix_ts = if opts.timestamp != 0 {
        opts.timestamp
    } else {
        opts.now_unix
    };
    if unix_ts < 0 {
        return Err(format!(
            "timestamp must be a non-negative Unix time in seconds, got {unix_ts}"
        ));
    }
    let signed_ts = if opts.legacy_epoch {
        if unix_ts < ITSDANGEROUS_LEGACY_EPOCH {
            return Err(format!(
                "legacy_epoch signs seconds since 2011-01-01 (Unix {ITSDANGEROUS_LEGACY_EPOCH}), so the timestamp cannot be earlier than that; got {unix_ts}"
            ));
        }
        unix_ts - ITSDANGEROUS_LEGACY_EPOCH
    } else {
        unix_ts
    };
    let timestamp_segment = b64url(&int_to_bytes(signed_ts));

    // 3. Derive the key and sign "<payload>.<timestamp>".
    let key = opts
        .key_derivation
        .derive(opts.digest, &secret_bytes, opts.salt.as_bytes());
    let signed_value = format!("{payload_segment}.{timestamp_segment}");
    let signature = opts.digest.hmac(&key, signed_value.as_bytes());
    let signature_segment = b64url(&signature);

    let cookie = format!("{signed_value}.{signature_segment}");
    let cookie_bytes = cookie_name.len() + 1 + cookie.len();

    let mut warnings = Vec::new();
    if cookie_bytes > BROWSER_COOKIE_LIMIT {
        warnings.push(format!(
            "{cookie_name}={{cookie}} is {cookie_bytes} bytes; browsers drop a single cookie over {BROWSER_COOKIE_LIMIT} bytes. Store less in the session, or move it server-side."
        ));
    }
    if opts.compress == CompressMode::Always && compressed_bytes.len() >= raw.len() {
        warnings.push(format!(
            "compress=always made the payload larger ({} bytes zlib vs {} raw); compress=auto would have skipped it.",
            compressed_bytes.len(),
            raw.len()
        ));
    }

    Ok(Signed {
        set_cookie_header: format!("{cookie_name}={cookie}; HttpOnly; Path=/; SameSite=Lax"),
        cookie,
        serialized_payload: serialized,
        payload_segment,
        timestamp_segment,
        signature_segment,
        compressed,
        timestamp: unix_ts,
        timestamp_iso: unix_to_iso(unix_ts),
        derived_key_hex: to_hex(&key),
        cookie_bytes,
        warnings,
    })
}

/// Sign and pretty-print to JSON (the surface-facing entry point).
pub fn sign_to_json(payload: &str, opts: &SignOptions) -> Result<String, String> {
    let signed = sign(payload, opts)?;
    serde_json::to_string_pretty(&signed).map_err(|e| format!("serialization failed: {e}"))
}

/// Convenience entry point for the simplest/page-facing mode: JSON payload +
/// UTF-8 secret using Flask defaults and the caller's current clock.
pub fn run(input: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(input)
        .map_err(|e| format!("input must be a JSON object with payload and secret fields: {e}"))?;
    let obj = value
        .as_object()
        .ok_or_else(|| "input must be a JSON object with payload and secret fields".to_string())?;
    let payload = obj.get("payload").and_then(|v| v.as_str()).ok_or_else(|| {
        "input.payload must be a JSON string containing the Flask session JSON".to_string()
    })?;
    let secret = obj
        .get("secret")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "input.secret must be a string".to_string())?;
    let now_unix = obj.get("now_unix").and_then(|v| v.as_i64()).unwrap_or(0);
    sign_to_json(
        payload,
        &SignOptions {
            secret: secret.to_string(),
            now_unix,
            ..Default::default()
        },
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    fn flask_defaults(secret: &str, timestamp: i64) -> SignOptions {
        SignOptions {
            secret: secret.to_string(),
            timestamp,
            ..Default::default()
        }
    }

    /// Ground truth from itsdangerous/Flask itself:
    ///   URLSafeTimedSerializer("CHANGEME", salt="cookie-session",
    ///       serializer=TaggedJSONSerializer(),
    ///       signer_kwargs={"key_derivation": "hmac", "digest_method": sha1})
    ///   with the clock pinned to 1547409146.
    /// Byte-for-byte the cookie Flask-Unsign's docs publish for that secret and
    /// payload, which is an independent implementation of the same algorithm.
    #[test]
    fn signs_flask_default_configuration() {
        let out = sign(
            r#"{"logged_in": true}"#,
            &flask_defaults("CHANGEME", 1_547_409_146),
        )
        .unwrap();
        assert_eq!(out.serialized_payload, r#"{"logged_in":true}"#);
        assert!(!out.compressed);
        assert_eq!(out.payload_segment, "eyJsb2dnZWRfaW4iOnRydWV9");
        assert_eq!(out.timestamp_segment, "XDuW-g");
        assert_eq!(out.signature_segment, "cPCkFmmeB7qNIcN-ReiN72r0hvU");
        assert_eq!(
            out.cookie,
            "eyJsb2dnZWRfaW4iOnRydWV9.XDuW-g.cPCkFmmeB7qNIcN-ReiN72r0hvU"
        );
        assert_eq!(out.timestamp_iso, "2019-01-13T19:52:26Z");
        assert_eq!(
            out.set_cookie_header,
            "session=eyJsb2dnZWRfaW4iOnRydWV9.XDuW-g.cPCkFmmeB7qNIcN-ReiN72r0hvU; HttpOnly; Path=/; SameSite=Lax"
        );
        assert!(out.warnings.is_empty());
    }

    /// The derived key is HMAC-SHA1(secret, b"cookie-session") — checkable
    /// against `hmac.new(b"CHANGEME", b"cookie-session", sha1).hexdigest()`.
    #[test]
    fn derives_flasks_hmac_key() {
        let out = sign(r#"{"a":1}"#, &flask_defaults("CHANGEME", 1_547_082_490)).unwrap();
        assert_eq!(
            out.derived_key_hex,
            "e1b8bcac89ead10301767f6702a3442db9f1121d"
        );
    }

    #[test]
    fn sorts_keys_and_uses_compact_separators() {
        let out = sign(
            r#"{ "zeta": 1, "alpha": [1, 2], "mid": {"b": 2, "a": 1} }"#,
            &flask_defaults("k", 1_700_000_000),
        )
        .unwrap();
        assert_eq!(
            out.serialized_payload,
            r#"{"alpha":[1,2],"mid":{"a":1,"b":2},"zeta":1}"#
        );
    }

    /// Flask's JSON provider defaults to `ensure_ascii=True`.
    #[test]
    fn escapes_non_ascii_like_python() {
        let out = sign(
            r#"{"user":"José","emoji":"🎂"}"#,
            &flask_defaults("k", 1_700_000_000),
        )
        .unwrap();
        assert_eq!(
            out.serialized_payload,
            r#"{"emoji":"\ud83c\udf82","user":"Jos\u00e9"}"#
        );
    }

    /// A one-key dict whose key collides with a TaggedJSONSerializer tag gets the
    /// `" di"` escape, exactly as Flask writes it.
    #[test]
    fn escapes_reserved_tag_dict() {
        let out = sign(r#"{" t":[1,2]}"#, &flask_defaults("k", 1_700_000_000)).unwrap();
        assert_eq!(out.serialized_payload, r#"{" di":{" t__":[1,2]}}"#);
        // A two-key dict is NOT a tag, so it stays as-is.
        let plain = sign(r#"{" t":1,"x":2}"#, &flask_defaults("k", 1_700_000_000)).unwrap();
        assert_eq!(plain.serialized_payload, r#"{" t":1,"x":2}"#);
    }

    #[test]
    fn auto_compresses_only_when_it_helps() {
        let small = sign(r#"{"a":1}"#, &flask_defaults("k", 1_700_000_000)).unwrap();
        assert!(!small.compressed, "tiny payloads must not be compressed");
        assert!(!small.payload_segment.starts_with('.'));

        let repetitive = format!(r#"{{"blob":"{}"}}"#, "ab".repeat(400));
        let big = sign(&repetitive, &flask_defaults("k", 1_700_000_000)).unwrap();
        assert!(big.compressed, "repetitive payloads must be compressed");
        assert!(big.payload_segment.starts_with('.'));
        assert!(big.cookie.len() < repetitive.len());
    }

    #[test]
    fn compress_modes_are_forced() {
        let repetitive = format!(r#"{{"blob":"{}"}}"#, "ab".repeat(400));
        let never = sign(
            &repetitive,
            &SignOptions {
                compress: CompressMode::Never,
                ..flask_defaults("k", 1_700_000_000)
            },
        )
        .unwrap();
        assert!(!never.compressed);

        let always = sign(
            r#"{"a":1}"#,
            &SignOptions {
                compress: CompressMode::Always,
                ..flask_defaults("k", 1_700_000_000)
            },
        )
        .unwrap();
        assert!(always.compressed);
        assert!(always.payload_segment.starts_with('.'));
        assert!(
            always.warnings.iter().any(|w| w.contains("larger")),
            "forcing compression on a tiny payload should warn: {:?}",
            always.warnings
        );
    }

    #[test]
    fn timestamp_zero_falls_back_to_the_supplied_clock() {
        let out = sign(
            r#"{"a":1}"#,
            &SignOptions {
                secret: "k".into(),
                timestamp: 0,
                now_unix: 1_700_000_000,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.timestamp, 1_700_000_000);
        assert_eq!(out.timestamp_iso, "2023-11-14T22:13:20Z");
    }

    /// itsdangerous < 1.0 signed an offset from 2011-01-01 instead of Unix time.
    #[test]
    fn legacy_epoch_signs_the_2011_offset() {
        let modern = sign(r#"{"a":1}"#, &flask_defaults("k", 1_700_000_000)).unwrap();
        let legacy = sign(
            r#"{"a":1}"#,
            &SignOptions {
                legacy_epoch: true,
                ..flask_defaults("k", 1_700_000_000)
            },
        )
        .unwrap();
        assert_ne!(modern.timestamp_segment, legacy.timestamp_segment);
        // Both report the same wall-clock time; only the encoded offset differs.
        assert_eq!(legacy.timestamp, 1_700_000_000);
        let raw = URL_SAFE_NO_PAD
            .decode(legacy.timestamp_segment.as_bytes())
            .unwrap();
        let mut n: i64 = 0;
        for b in raw {
            n = (n << 8) | i64::from(b);
        }
        assert_eq!(n, 1_700_000_000 - ITSDANGEROUS_LEGACY_EPOCH);
    }

    #[test]
    fn digest_and_derivation_change_the_signature() {
        let base = sign(r#"{"a":1}"#, &flask_defaults("k", 1_700_000_000)).unwrap();
        let sha256 = sign(
            r#"{"a":1}"#,
            &SignOptions {
                digest: DigestMethod::Sha256,
                ..flask_defaults("k", 1_700_000_000)
            },
        )
        .unwrap();
        let django = sign(
            r#"{"a":1}"#,
            &SignOptions {
                key_derivation: KeyDerivation::DjangoConcat,
                ..flask_defaults("k", 1_700_000_000)
            },
        )
        .unwrap();
        let none = sign(
            r#"{"a":1}"#,
            &SignOptions {
                key_derivation: KeyDerivation::None,
                ..flask_defaults("k", 1_700_000_000)
            },
        )
        .unwrap();
        assert_ne!(base.signature_segment, sha256.signature_segment);
        assert_ne!(base.signature_segment, django.signature_segment);
        assert_ne!(base.derived_key_hex, django.derived_key_hex);
        // key_derivation=none uses the secret verbatim.
        assert_eq!(none.derived_key_hex, to_hex(b"k"));
        // SHA-256 signatures are 32 bytes → 43 base64url chars.
        assert_eq!(sha256.signature_segment.len(), 43);
        assert_eq!(base.signature_segment.len(), 27);
    }

    #[test]
    fn secret_encodings_agree_on_the_same_key_bytes() {
        let utf8 = sign(
            r#"{"a":1}"#,
            &SignOptions {
                secret: "AB".into(),
                ..flask_defaults("AB", 1_700_000_000)
            },
        )
        .unwrap();
        let hex = sign(
            r#"{"a":1}"#,
            &SignOptions {
                secret: "4142".into(),
                secret_encoding: SecretEncoding::Hex,
                ..flask_defaults("x", 1_700_000_000)
            },
        )
        .unwrap();
        let b64 = sign(
            r#"{"a":1}"#,
            &SignOptions {
                secret: "QUI=".into(),
                secret_encoding: SecretEncoding::Base64,
                ..flask_defaults("x", 1_700_000_000)
            },
        )
        .unwrap();
        assert_eq!(utf8.cookie, hex.cookie);
        assert_eq!(utf8.cookie, b64.cookie);
    }

    #[test]
    fn custom_salt_and_cookie_name_are_applied() {
        let out = sign(
            r#"{"a":1}"#,
            &SignOptions {
                salt: "my-salt".into(),
                cookie_name: "my_session".into(),
                ..flask_defaults("k", 1_700_000_000)
            },
        )
        .unwrap();
        let default = sign(r#"{"a":1}"#, &flask_defaults("k", 1_700_000_000)).unwrap();
        assert_ne!(out.derived_key_hex, default.derived_key_hex);
        assert!(out.set_cookie_header.starts_with("my_session="));
    }

    #[test]
    fn warns_when_the_cookie_exceeds_the_browser_limit() {
        // Random-ish, incompressible content so it stays big after zlib.
        let filler: String = (0..5000u32)
            .map(|i| char::from(b'a' + ((i * 7 + i / 13) % 26) as u8))
            .collect();
        let out = sign(
            &format!(r#"{{"blob":"{filler}"}}"#),
            &SignOptions {
                compress: CompressMode::Never,
                ..flask_defaults("k", 1_700_000_000)
            },
        )
        .unwrap();
        assert!(out.cookie_bytes > BROWSER_COOKIE_LIMIT);
        assert!(out.warnings.iter().any(|w| w.contains("browsers drop")));
    }

    #[test]
    fn payload_at_the_cap_is_accepted_and_one_over_is_rejected() {
        let overhead = r#"{"b":""}"#.len();
        let at_cap = format!(r#"{{"b":"{}"}}"#, "x".repeat(MAX_PAYLOAD_BYTES - overhead));
        assert_eq!(at_cap.len(), MAX_PAYLOAD_BYTES);
        assert!(sign(&at_cap, &flask_defaults("k", 1_700_000_000)).is_ok());

        let over = format!(
            r#"{{"b":"{}"}}"#,
            "x".repeat(MAX_PAYLOAD_BYTES - overhead + 1)
        );
        let err = sign(&over, &flask_defaults("k", 1_700_000_000)).unwrap_err();
        assert!(err.contains("over the"), "{err}");
    }

    #[test]
    fn rejects_invalid_json_payload() {
        let err = sign("{'logged_in': True}", &flask_defaults("k", 1_700_000_000)).unwrap_err();
        assert!(err.contains("not valid JSON"), "{err}");
        assert!(err.contains("True/False/None"), "{err}");
    }

    #[test]
    fn rejects_non_object_payload() {
        let err = sign("[1,2,3]", &flask_defaults("k", 1_700_000_000)).unwrap_err();
        assert!(err.contains("must be a JSON object"), "{err}");
        assert!(err.contains("an array"), "{err}");
    }

    #[test]
    fn rejects_empty_payload_and_secret() {
        assert!(sign("   ", &flask_defaults("k", 1))
            .unwrap_err()
            .contains("empty payload"));
        assert!(sign(r#"{"a":1}"#, &SignOptions::default())
            .unwrap_err()
            .contains("empty secret"));
    }

    #[test]
    fn rejects_bad_option_values() {
        assert!(DigestMethod::parse("md5").unwrap_err().contains("sha1"));
        assert!(KeyDerivation::parse("pbkdf2")
            .unwrap_err()
            .contains("django-concat"));
        assert!(SecretEncoding::parse("base32").unwrap_err().contains("hex"));
        assert!(CompressMode::parse("maybe").unwrap_err().contains("auto"));
        let err = SecretEncoding::Hex.decode("abc").unwrap_err();
        assert!(err.contains("even number of hex digits"), "{err}");
        let err = SecretEncoding::Hex.decode("zz").unwrap_err();
        assert!(err.contains("not a hex byte"), "{err}");
    }

    #[test]
    fn rejects_bad_cookie_name_and_negative_timestamp() {
        let err = sign(
            r#"{"a":1}"#,
            &SignOptions {
                cookie_name: "bad name".into(),
                ..flask_defaults("k", 1_700_000_000)
            },
        )
        .unwrap_err();
        assert!(err.contains("cookie_name may only contain"), "{err}");

        let err = sign(r#"{"a":1}"#, &flask_defaults("k", -5)).unwrap_err();
        assert!(err.contains("non-negative Unix time"), "{err}");

        let err = sign(
            r#"{"a":1}"#,
            &SignOptions {
                legacy_epoch: true,
                ..flask_defaults("k", 1_000_000)
            },
        )
        .unwrap_err();
        assert!(err.contains("cannot be earlier"), "{err}");
    }

    #[test]
    fn int_to_bytes_is_minimal_big_endian() {
        assert_eq!(int_to_bytes(0), Vec::<u8>::new());
        assert_eq!(int_to_bytes(255), vec![0xFF]);
        assert_eq!(int_to_bytes(256), vec![0x01, 0x00]);
        assert_eq!(int_to_bytes(1_547_082_490), vec![0x5C, 0x36, 0x9A, 0xFA]);
    }

    #[test]
    fn json_output_is_pretty_and_complete() {
        let json = sign_to_json(r#"{"a":1}"#, &flask_defaults("k", 1_700_000_000)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        for key in [
            "cookie",
            "set_cookie_header",
            "serialized_payload",
            "payload_segment",
            "timestamp_segment",
            "signature_segment",
            "compressed",
            "timestamp",
            "timestamp_iso",
            "derived_key_hex",
            "cookie_bytes",
            "warnings",
        ] {
            assert!(v.get(key).is_some(), "missing '{key}' in output");
        }
    }
}
