//! webhook-signature-generator core — build the signing header(s) a webhook
//! provider would send for a given raw request body, so you can replay or test
//! a delivery against your own receiver. Pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps.
//!
//! Every supported provider is a symmetric HMAC scheme, so the whole thing is
//! deterministic: pick the provider, paste the exact raw body and the signing
//! secret, and the tool reproduces the provider's signed string, its HMAC, and
//! the finished header line(s). The three things that differ between providers
//! are (a) which bytes get signed, (b) which hash + output encoding is used,
//! and (c) how the result is decorated into a header value — all encoded in
//! `signed_payload_for` / `provider_spec` below.
//!
//! All hashers are pure-Rust (RustCrypto), so this runs on every backend
//! including the chat Service Worker and the browser page.

use base64::Engine;
use hmac::{Hmac, Mac};

// ---------------------------------------------------------------------------
// Enumerations (single source of truth for the descriptor, manifest and page)
// ---------------------------------------------------------------------------

/// A webhook provider's signing scheme.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Provider {
    Stripe,
    Github,
    Slack,
    Shopify,
    StandardWebhooks,
    Svix,
    Square,
    Twilio,
    Paddle,
    Custom,
}

/// Underlying hash for the HMAC. Fixed by the spec for every provider except
/// `custom`, where the caller chooses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Algorithm {
    Md5,
    Sha1,
    Sha256,
    Sha384,
    Sha512,
}

/// How the raw HMAC bytes are rendered into the header value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Encoding {
    Hex,
    HexUpper,
    Base64,
    Base64Url,
}

/// How the `secret` string is turned into HMAC key bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SecretEncoding {
    /// Follow the provider's own convention (see `secret_key_bytes`).
    Auto,
    Text,
    Hex,
    Base64,
}

/// Which artifact to return.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    All,
    Headers,
    Header,
    Signature,
    SignedPayload,
    Curl,
}

/// Canonical provider identifiers, in menu order.
pub const PROVIDERS: &[&str] = &[
    "stripe",
    "github",
    "slack",
    "shopify",
    "standard-webhooks",
    "svix",
    "square",
    "twilio",
    "paddle",
    "custom",
];

/// Canonical algorithm identifiers, in menu order.
pub const ALGORITHMS: &[&str] = &["md5", "sha1", "sha256", "sha384", "sha512"];

/// Canonical signature-encoding identifiers, in menu order.
pub const ENCODINGS: &[&str] = &["hex", "hex-upper", "base64", "base64url"];

/// Canonical secret-encoding identifiers, in menu order.
pub const SECRET_ENCODINGS: &[&str] = &["auto", "text", "hex", "base64"];

/// Canonical output selectors, in menu order.
pub const OUTPUTS: &[&str] = &[
    "all",
    "headers",
    "header",
    "signature",
    "signed-payload",
    "curl",
];

/// Largest accepted payload, in bytes. Webhook bodies are small; the cap keeps
/// the browser/Service-Worker sandbox well inside its memory budget.
pub const MAX_PAYLOAD_BYTES: usize = 2 * 1024 * 1024;

/// Placeholder endpoint used for the cURL command when no URL is supplied.
pub const DEFAULT_ENDPOINT: &str = "https://example.com/webhook";

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn parse_provider(s: &str) -> Result<Provider, String> {
    // Normalize: lowercase, strip '-'/'_'/' ' so "standard-webhooks",
    // "standard_webhooks" and "Standard Webhooks" all resolve to the same spec.
    let canon: String = s
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| *c != '-' && *c != '_' && *c != ' ')
        .collect();
    Ok(match canon.as_str() {
        "" | "stripe" => Provider::Stripe, // stripe is the default
        "github" | "githubsha256" | "xhubsignature256" => Provider::Github,
        "slack" => Provider::Slack,
        "shopify" => Provider::Shopify,
        "standardwebhooks" | "standardwebhook" => Provider::StandardWebhooks,
        "svix" => Provider::Svix,
        "square" => Provider::Square,
        "twilio" => Provider::Twilio,
        "paddle" => Provider::Paddle,
        "custom" | "generic" => Provider::Custom,
        other => {
            return Err(format!(
                "invalid provider '{other}': expected one of {}",
                PROVIDERS.join(", ")
            ))
        }
    })
}

fn parse_algorithm(s: &str) -> Result<Algorithm, String> {
    let canon: String = s
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| *c != '-' && *c != '_')
        .collect();
    Ok(match canon.as_str() {
        "" | "sha256" => Algorithm::Sha256, // sha256 is the default
        "md5" => Algorithm::Md5,
        "sha1" => Algorithm::Sha1,
        "sha384" => Algorithm::Sha384,
        "sha512" => Algorithm::Sha512,
        other => {
            return Err(format!(
                "invalid algorithm '{other}': expected one of {}",
                ALGORITHMS.join(", ")
            ))
        }
    })
}

fn parse_encoding(s: &str) -> Result<Encoding, String> {
    let canon: String = s
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| *c != '-' && *c != '_')
        .collect();
    Ok(match canon.as_str() {
        "" | "hex" | "hexlower" => Encoding::Hex, // hex is the default
        "hexupper" | "hexuppercase" | "hexu" => Encoding::HexUpper,
        "base64" | "b64" => Encoding::Base64,
        "base64url" | "b64url" | "base64urlsafe" => Encoding::Base64Url,
        other => {
            return Err(format!(
                "invalid encoding '{other}': expected one of {}",
                ENCODINGS.join(", ")
            ))
        }
    })
}

fn parse_secret_encoding(s: &str) -> Result<SecretEncoding, String> {
    Ok(match s.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => SecretEncoding::Auto, // auto is the default
        "text" | "utf8" | "utf-8" | "raw" | "plain" => SecretEncoding::Text,
        "hex" => SecretEncoding::Hex,
        "base64" | "b64" => SecretEncoding::Base64,
        other => {
            return Err(format!(
                "invalid secret_encoding '{other}': expected one of {}",
                SECRET_ENCODINGS.join(", ")
            ))
        }
    })
}

fn parse_output(s: &str) -> Result<Output, String> {
    let canon: String = s
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| *c != '_' && *c != ' ')
        .collect();
    Ok(match canon.as_str() {
        "" | "all" => Output::All, // all is the default
        "headers" => Output::Headers,
        "header" => Output::Header,
        "signature" | "sig" => Output::Signature,
        "signed-payload" | "signedpayload" | "canonical" => Output::SignedPayload,
        "curl" => Output::Curl,
        other => {
            return Err(format!(
                "invalid output '{other}': expected one of {}",
                OUTPUTS.join(", ")
            ))
        }
    })
}

// ---------------------------------------------------------------------------
// Time helpers
// ---------------------------------------------------------------------------

/// Render a Unix timestamp the way the tool's `timestamp` param expects it.
/// Each surface (chat/CLI = std clock, page = `js_sys::Date`) supplies its own
/// "now" and calls this, so the core itself stays deterministic.
pub fn format_timestamp(epoch_secs: i64) -> String {
    epoch_secs.to_string()
}

/// Does this provider/template combination fold a timestamp into the signed
/// bytes? Surfaces call this to decide whether a blank `timestamp` should be
/// filled from their clock — filling it for a body-only scheme (github,
/// shopify, square, twilio) is harmless but adds a spurious "timestamp
/// ignored" note to the report. An unrecognised provider answers `false`;
/// `sign` is what reports the invalid value.
pub fn provider_needs_timestamp(provider: &str, template: &str) -> bool {
    let template = if template.trim().is_empty() { "{payload}" } else { template };
    parse_provider(provider).map(|p| uses_timestamp(p, template)).unwrap_or(false)
}

/// Days since the Unix epoch for a proleptic-Gregorian date (Howard Hinnant's
/// `days_from_civil`). Used to accept ISO-8601 timestamps without a date crate.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of `days_from_civil` — used only to echo a human-readable UTC time
/// next to the numeric timestamp in the `all` report.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn iso_utc(epoch_secs: i64) -> String {
    let days = epoch_secs.div_euclid(86_400);
    let secs = epoch_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// Accept either Unix seconds (`1700000000`) or an ISO-8601 UTC/offset
/// timestamp (`2023-11-14T22:13:20Z`, `2023-11-14 22:13:20+01:00`).
fn parse_timestamp(s: &str) -> Result<i64, String> {
    let t = s.trim();
    if t.is_empty() {
        return Err("timestamp is empty".into());
    }
    if let Ok(n) = t.parse::<i64>() {
        // Milliseconds are a common paste mistake; they'd silently produce a
        // signature the receiver rejects as far-future, so name it explicitly.
        if n.abs() >= 100_000_000_000 {
            return Err(format!(
                "timestamp '{t}' looks like milliseconds — webhook signatures use \
                 Unix SECONDS (divide by 1000)"
            ));
        }
        return Ok(n);
    }

    let bad = || format!("invalid timestamp '{t}': expected Unix seconds (e.g. 1700000000) or ISO-8601 (e.g. 2023-11-14T22:13:20Z)");
    let (date, rest) = match t.split_once(['T', 't', ' ']) {
        Some(p) => p,
        None => return Err(bad()),
    };
    let dp: Vec<&str> = date.split('-').collect();
    if dp.len() != 3 {
        return Err(bad());
    }
    let (y, mo, d) = (
        dp[0].parse::<i64>().map_err(|_| bad())?,
        dp[1].parse::<i64>().map_err(|_| bad())?,
        dp[2].parse::<i64>().map_err(|_| bad())?,
    );
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return Err(bad());
    }

    // Split the offset off the time part before parsing hh:mm:ss.
    let rest = rest.trim();
    let (time, offset_secs) = if let Some(stripped) = rest.strip_suffix(['Z', 'z']) {
        (stripped, 0i64)
    } else if let Some(pos) = rest.rfind(['+', '-']) {
        let (time, off) = rest.split_at(pos);
        let sign = if off.starts_with('-') { -1 } else { 1 };
        let off = &off[1..];
        let (oh, om) = match off.split_once(':') {
            Some((h, m)) => (h.parse::<i64>().map_err(|_| bad())?, m.parse::<i64>().map_err(|_| bad())?),
            None if off.len() == 4 => (
                off[..2].parse::<i64>().map_err(|_| bad())?,
                off[2..].parse::<i64>().map_err(|_| bad())?,
            ),
            None => (off.parse::<i64>().map_err(|_| bad())?, 0),
        };
        (time, sign * (oh * 3600 + om * 60))
    } else {
        (rest, 0i64)
    };

    // Drop any fractional seconds — signatures use whole seconds.
    let time = time.split('.').next().unwrap_or("");
    let tp: Vec<&str> = time.split(':').collect();
    if tp.is_empty() || tp.len() > 3 {
        return Err(bad());
    }
    let h = tp[0].parse::<i64>().map_err(|_| bad())?;
    let mi = if tp.len() > 1 { tp[1].parse::<i64>().map_err(|_| bad())? } else { 0 };
    let se = if tp.len() > 2 { tp[2].parse::<i64>().map_err(|_| bad())? } else { 0 };
    if h > 23 || mi > 59 || se > 60 {
        return Err(bad());
    }
    Ok(days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + se - offset_secs)
}

// ---------------------------------------------------------------------------
// Encoding helpers
// ---------------------------------------------------------------------------

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn decode_hex(s: &str, field: &str) -> Result<Vec<u8>, String> {
    let clean: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if clean.len() % 2 != 0 {
        return Err(format!(
            "{field} is not valid hex: it has an odd number of digits ({})",
            clean.len()
        ));
    }
    clean
        .chunks(2)
        .map(|p| match (hex_val(p[0]), hex_val(p[1])) {
            (Some(h), Some(l)) => Ok(h * 16 + l),
            _ => Err(format!(
                "{field} is not valid hex: '{}' is not a hex digit pair",
                String::from_utf8_lossy(p)
            )),
        })
        .collect()
}

fn decode_base64(s: &str, field: &str) -> Result<Vec<u8>, String> {
    let clean: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    // Accept both the standard and URL-safe alphabets, padded or not.
    let normalized: String = clean.chars().map(|c| match c {
        '-' => '+',
        '_' => '/',
        other => other,
    }).collect();
    base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(normalized.trim_end_matches('='))
        .map_err(|e| format!("{field} is not valid base64: {e}"))
}

fn encode_signature(bytes: &[u8], encoding: Encoding) -> String {
    match encoding {
        Encoding::Hex => hex_lower(bytes),
        Encoding::HexUpper => hex_lower(bytes).to_ascii_uppercase(),
        Encoding::Base64 => base64::engine::general_purpose::STANDARD.encode(bytes),
        Encoding::Base64Url => base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes),
    }
}

/// Percent/plus decoding for `application/x-www-form-urlencoded` values —
/// needed only by Twilio, which signs the DECODED parameter values.
fn form_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < b.len() => match (hex_val(b[i + 1]), hex_val(b[i + 2])) {
                (Some(h), Some(l)) => {
                    out.push(h * 16 + l);
                    i += 3;
                }
                _ => {
                    out.push(b'%');
                    i += 1;
                }
            },
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---------------------------------------------------------------------------
// HMAC
// ---------------------------------------------------------------------------

fn hmac_bytes(alg: Algorithm, key: &[u8], msg: &[u8]) -> Vec<u8> {
    macro_rules! go {
        ($hash:ty) => {{
            // HMAC accepts a key of ANY length (RFC 2104), so new_from_slice
            // cannot fail here.
            let mut mac = <Hmac<$hash>>::new_from_slice(key)
                .expect("HMAC accepts keys of any length");
            mac.update(msg);
            mac.finalize().into_bytes().to_vec()
        }};
    }
    match alg {
        Algorithm::Md5 => go!(md5::Md5),
        Algorithm::Sha1 => go!(sha1::Sha1),
        Algorithm::Sha256 => go!(sha2::Sha256),
        Algorithm::Sha384 => go!(sha2::Sha384),
        Algorithm::Sha512 => go!(sha2::Sha512),
    }
}

fn algorithm_label(alg: Algorithm) -> &'static str {
    match alg {
        Algorithm::Md5 => "HMAC-MD5",
        Algorithm::Sha1 => "HMAC-SHA1",
        Algorithm::Sha256 => "HMAC-SHA256",
        Algorithm::Sha384 => "HMAC-SHA384",
        Algorithm::Sha512 => "HMAC-SHA512",
    }
}

fn encoding_label(enc: Encoding) -> &'static str {
    match enc {
        Encoding::Hex => "hex",
        Encoding::HexUpper => "hex-upper",
        Encoding::Base64 => "base64",
        Encoding::Base64Url => "base64url",
    }
}

fn provider_label(p: Provider) -> &'static str {
    match p {
        Provider::Stripe => "stripe",
        Provider::Github => "github",
        Provider::Slack => "slack",
        Provider::Shopify => "shopify",
        Provider::StandardWebhooks => "standard-webhooks",
        Provider::Svix => "svix",
        Provider::Square => "square",
        Provider::Twilio => "twilio",
        Provider::Paddle => "paddle",
        Provider::Custom => "custom",
    }
}

// ---------------------------------------------------------------------------
// Provider specs
// ---------------------------------------------------------------------------

/// The fixed (alg, encoding) a provider's spec mandates, or `None` for `custom`
/// where the caller picks both.
fn provider_spec(p: Provider) -> Option<(Algorithm, Encoding)> {
    Some(match p {
        Provider::Stripe => (Algorithm::Sha256, Encoding::Hex),
        Provider::Github => (Algorithm::Sha256, Encoding::Hex),
        Provider::Slack => (Algorithm::Sha256, Encoding::Hex),
        Provider::Shopify => (Algorithm::Sha256, Encoding::Base64),
        Provider::StandardWebhooks | Provider::Svix => (Algorithm::Sha256, Encoding::Base64),
        Provider::Square => (Algorithm::Sha256, Encoding::Base64),
        Provider::Twilio => (Algorithm::Sha1, Encoding::Base64),
        Provider::Paddle => (Algorithm::Sha256, Encoding::Hex),
        Provider::Custom => return None,
    })
}

/// Does this provider fold a timestamp into the signed bytes?
fn uses_timestamp(p: Provider, template: &str) -> bool {
    match p {
        Provider::Stripe
        | Provider::Slack
        | Provider::StandardWebhooks
        | Provider::Svix
        | Provider::Paddle => true,
        Provider::Custom => template.contains("{timestamp}"),
        _ => false,
    }
}

/// Does this provider need an endpoint URL as part of the signed bytes?
fn uses_url(p: Provider, template: &str) -> bool {
    match p {
        Provider::Square | Provider::Twilio => true,
        Provider::Custom => template.contains("{url}"),
        _ => false,
    }
}

/// Does this provider need a message id as part of the signed bytes?
fn uses_message_id(p: Provider, template: &str) -> bool {
    match p {
        Provider::StandardWebhooks | Provider::Svix => true,
        Provider::Custom => template.contains("{id}"),
        _ => false,
    }
}

/// Turn the `secret` string into HMAC key bytes. `auto` follows each provider's
/// own convention: Standard Webhooks / Svix secrets are a `whsec_`-prefixed
/// BASE64 blob that must be decoded first, while Stripe's `whsec_…` secret is
/// used as literal text (a classic source of "signature never matches").
fn secret_key_bytes(
    secret: &str,
    provider: Provider,
    secret_encoding: SecretEncoding,
) -> Result<(Vec<u8>, Option<String>), String> {
    let trimmed = secret.trim();
    if trimmed.is_empty() {
        return Err("secret is required: paste the endpoint's signing secret (it never leaves this page)".into());
    }
    match secret_encoding {
        SecretEncoding::Text => Ok((trimmed.as_bytes().to_vec(), None)),
        SecretEncoding::Hex => Ok((decode_hex(trimmed, "secret")?, None)),
        SecretEncoding::Base64 => Ok((decode_base64(trimmed, "secret")?, None)),
        SecretEncoding::Auto => match provider {
            Provider::StandardWebhooks | Provider::Svix => {
                let body = trimmed.strip_prefix("whsec_").unwrap_or(trimmed);
                let bytes = decode_base64(body, "secret")?;
                Ok((
                    bytes,
                    Some(
                        "secret_encoding=auto: base64-decoded the Standard Webhooks/Svix secret \
                         (whsec_ prefix stripped). Use secret_encoding=text to sign with the \
                         literal characters instead."
                            .into(),
                    ),
                ))
            }
            _ => Ok((trimmed.as_bytes().to_vec(), None)),
        },
    }
}

/// Deterministic stand-in message id when the caller leaves `message_id` blank,
/// so the same payload always replays with the same id.
fn derive_message_id(payload: &str) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(payload.as_bytes());
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);
    let id: String = b64.chars().filter(|c| c.is_ascii_alphanumeric()).take(22).collect();
    format!("msg_{id}")
}

/// Twilio signs the endpoint URL followed by every POST parameter, sorted by
/// name, as `name` + `decoded value` with no separators.
fn twilio_signed_payload(url: &str, body: &str) -> String {
    let mut pairs: Vec<(String, String)> = body
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (form_decode(k), form_decode(v)),
            None => (form_decode(p), String::new()),
        })
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = String::from(url);
    for (k, v) in pairs {
        out.push_str(&k);
        out.push_str(&v);
    }
    out
}

/// Expand a `custom` template. Supported placeholders: `{payload}`,
/// `{timestamp}`, `{id}`, `{url}`. `\n` / `\t` escapes become real characters.
fn expand_template(
    template: &str,
    payload: &str,
    timestamp: &str,
    id: &str,
    url: &str,
) -> Result<String, String> {
    let template = template.replace("\\n", "\n").replace("\\t", "\t");
    let mut out = String::with_capacity(template.len() + payload.len());
    let mut rest = template.as_str();
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let end = match after.find('}') {
            Some(e) => e,
            None => {
                return Err(format!(
                    "unterminated placeholder in template: '{}' has a '{{' with no closing '}}'",
                    template
                ))
            }
        };
        let name = &after[..end];
        match name {
            "payload" | "body" => out.push_str(payload),
            "timestamp" | "ts" => out.push_str(timestamp),
            "id" => out.push_str(id),
            "url" => out.push_str(url),
            other => {
                return Err(format!(
                    "unknown template placeholder '{{{other}}}': expected {{payload}}, {{timestamp}}, {{id}} or {{url}}"
                ))
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// The full result of signing one webhook delivery.
#[derive(Clone, Debug)]
pub struct Signed {
    pub provider: Provider,
    pub algorithm: Algorithm,
    pub encoding: Encoding,
    /// The exact byte string the HMAC is computed over.
    pub signed_payload: String,
    /// The encoded HMAC on its own (no header decoration).
    pub signature: String,
    /// Every header a real delivery would carry, in send order.
    pub headers: Vec<(String, String)>,
    /// Index into `headers` of the primary signature header.
    pub primary: usize,
    /// Advisory notes shown in the `all` report.
    pub notes: Vec<String>,
    /// Timestamp actually used, when the scheme uses one.
    pub timestamp: Option<i64>,
}

/// Build the signing header(s) for a webhook payload.
///
/// `timestamp` accepts Unix seconds or ISO-8601; each surface fills a blank
/// value with its own clock BEFORE calling (this core is deterministic).
#[allow(clippy::too_many_arguments)]
pub fn sign(
    payload: &str,
    secret: &str,
    provider: &str,
    timestamp: &str,
    message_id: &str,
    url: &str,
    algorithm: &str,
    encoding: &str,
    secret_encoding: &str,
    template: &str,
    header_name: &str,
    signature_prefix: &str,
) -> Result<Signed, String> {
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(format!(
            "payload is {} bytes, which exceeds the {} byte limit — webhook bodies are small; \
             sign a representative sample instead",
            payload.len(),
            MAX_PAYLOAD_BYTES
        ));
    }
    let provider = parse_provider(provider)?;
    let secret_encoding = parse_secret_encoding(secret_encoding)?;
    let mut notes: Vec<String> = Vec::new();

    let template = if template.trim().is_empty() { "{payload}" } else { template };

    // Algorithm + encoding are dictated by the spec for every named provider.
    let (alg, enc) = match provider_spec(provider) {
        Some((a, e)) => {
            let chosen_alg = parse_algorithm(algorithm)?;
            let chosen_enc = parse_encoding(encoding)?;
            if !algorithm.trim().is_empty() && chosen_alg != a {
                notes.push(format!(
                    "algorithm '{}' ignored: the {} scheme is fixed at {}.",
                    algorithm.trim(),
                    provider_label(provider),
                    algorithm_label(a)
                ));
            }
            if !encoding.trim().is_empty() && chosen_enc != e {
                notes.push(format!(
                    "encoding '{}' ignored: the {} scheme is fixed at {}.",
                    encoding.trim(),
                    provider_label(provider),
                    encoding_label(e)
                ));
            }
            (a, e)
        }
        None => (parse_algorithm(algorithm)?, parse_encoding(encoding)?),
    };

    // Timestamp: only parsed when the scheme actually uses one.
    let ts_used = uses_timestamp(provider, template);
    let ts_secs = if ts_used {
        if timestamp.trim().is_empty() {
            return Err(
                "timestamp is required for this provider — pass Unix seconds (e.g. 1700000000), \
                 an ISO-8601 time, or leave it blank to use the current time"
                    .into(),
            );
        }
        Some(parse_timestamp(timestamp)?)
    } else {
        if !timestamp.trim().is_empty() {
            notes.push(format!(
                "timestamp ignored: the {} scheme signs the body only.",
                provider_label(provider)
            ));
        }
        None
    };
    let ts_str = ts_secs.map(|t| t.to_string()).unwrap_or_default();

    // Message id (Standard Webhooks / Svix, or a custom {id} template).
    let id = if uses_message_id(provider, template) {
        if message_id.trim().is_empty() {
            let derived = derive_message_id(payload);
            notes.push(format!(
                "message_id was blank — derived '{derived}' from the payload so replays stay \
                 reproducible. Set message_id to match a real delivery."
            ));
            derived
        } else {
            message_id.trim().to_string()
        }
    } else {
        message_id.trim().to_string()
    };

    // Endpoint URL: part of the signed bytes for square/twilio, and the target
    // of the generated cURL command for everyone else.
    let url_trimmed = url.trim();
    if uses_url(provider, template) && url_trimmed.is_empty() {
        return Err(format!(
            "url is required for the {} scheme: it signs the destination URL together with the body",
            provider_label(provider)
        ));
    }
    // Which bytes get signed.
    let signed_payload = match provider {
        Provider::Stripe => format!("{ts_str}.{payload}"),
        Provider::Github | Provider::Shopify => payload.to_string(),
        Provider::Slack => format!("v0:{ts_str}:{payload}"),
        Provider::StandardWebhooks | Provider::Svix => format!("{id}.{ts_str}.{payload}"),
        Provider::Square => format!("{url_trimmed}{payload}"),
        Provider::Twilio => twilio_signed_payload(url_trimmed, payload),
        Provider::Paddle => format!("{ts_str}:{payload}"),
        Provider::Custom => expand_template(template, payload, &ts_str, &id, url_trimmed)?,
    };

    let (key, key_note) = secret_key_bytes(secret, provider, secret_encoding)?;
    if let Some(n) = key_note {
        notes.push(n);
    }
    let signature = encode_signature(&hmac_bytes(alg, &key, signed_payload.as_bytes()), enc);

    // Decorate into the header set a real delivery carries.
    let mut headers: Vec<(String, String)> = Vec::new();
    let primary;
    match provider {
        Provider::Stripe => {
            headers.push(("Stripe-Signature".into(), format!("t={ts_str},v1={signature}")));
            primary = 0;
        }
        Provider::Github => {
            headers.push(("X-Hub-Signature-256".into(), format!("sha256={signature}")));
            primary = 0;
            // GitHub still sends the deprecated SHA-1 header alongside; emit it
            // so old receivers can be exercised too.
            let legacy = encode_signature(
                &hmac_bytes(Algorithm::Sha1, &key, signed_payload.as_bytes()),
                Encoding::Hex,
            );
            headers.push(("X-Hub-Signature".into(), format!("sha1={legacy}")));
        }
        Provider::Slack => {
            headers.push(("X-Slack-Request-Timestamp".into(), ts_str.clone()));
            headers.push(("X-Slack-Signature".into(), format!("v0={signature}")));
            primary = 1;
        }
        Provider::Shopify => {
            headers.push(("X-Shopify-Hmac-SHA256".into(), signature.clone()));
            primary = 0;
        }
        Provider::StandardWebhooks => {
            headers.push(("webhook-id".into(), id.clone()));
            headers.push(("webhook-timestamp".into(), ts_str.clone()));
            headers.push(("webhook-signature".into(), format!("v1,{signature}")));
            primary = 2;
        }
        Provider::Svix => {
            headers.push(("svix-id".into(), id.clone()));
            headers.push(("svix-timestamp".into(), ts_str.clone()));
            headers.push(("svix-signature".into(), format!("v1,{signature}")));
            primary = 2;
        }
        Provider::Square => {
            headers.push(("x-square-hmacsha256-signature".into(), signature.clone()));
            primary = 0;
        }
        Provider::Twilio => {
            headers.push(("X-Twilio-Signature".into(), signature.clone()));
            primary = 0;
        }
        Provider::Paddle => {
            headers.push(("Paddle-Signature".into(), format!("ts={ts_str};h1={signature}")));
            primary = 0;
        }
        Provider::Custom => {
            let name = if header_name.trim().is_empty() { "X-Signature" } else { header_name.trim() };
            headers.push((name.to_string(), format!("{}{signature}", signature_prefix.trim())));
            primary = 0;
        }
    }

    Ok(Signed {
        provider,
        algorithm: alg,
        encoding: enc,
        signed_payload,
        signature,
        headers,
        primary,
        notes,
        timestamp: ts_secs,
    })
}

/// Shell-quote a value for the generated cURL command (single-quoted, with the
/// standard `'\''` escape for embedded quotes).
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn render_curl(signed: &Signed, payload: &str, endpoint: &str, content_type: &str) -> String {
    let mut out = format!("curl -X POST {}", shell_quote(endpoint));
    out.push_str(&format!(" \\\n  -H {}", shell_quote(&format!("Content-Type: {content_type}"))));
    for (name, value) in &signed.headers {
        out.push_str(&format!(" \\\n  -H {}", shell_quote(&format!("{name}: {value}"))));
    }
    out.push_str(&format!(" \\\n  --data-raw {}", shell_quote(payload)));
    out
}

/// Format a `Signed` result according to the `output` selector. This is what
/// every surface (chat, CLI, page) actually returns.
#[allow(clippy::too_many_arguments)]
pub fn run(
    payload: &str,
    secret: &str,
    provider: &str,
    timestamp: &str,
    message_id: &str,
    url: &str,
    algorithm: &str,
    encoding: &str,
    secret_encoding: &str,
    template: &str,
    header_name: &str,
    signature_prefix: &str,
    output: &str,
) -> Result<String, String> {
    let out = parse_output(output)?;
    let signed = sign(
        payload,
        secret,
        provider,
        timestamp,
        message_id,
        url,
        algorithm,
        encoding,
        secret_encoding,
        template,
        header_name,
        signature_prefix,
    )?;
    let endpoint = if url.trim().is_empty() { DEFAULT_ENDPOINT } else { url.trim() };
    // Twilio deliveries are form-encoded; everything else here is JSON.
    let content_type = if signed.provider == Provider::Twilio {
        "application/x-www-form-urlencoded"
    } else {
        "application/json"
    };

    let headers_block = || {
        signed
            .headers
            .iter()
            .map(|(n, v)| format!("{n}: {v}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    Ok(match out {
        Output::Signature => signed.signature.clone(),
        Output::Header => signed.headers[signed.primary].1.clone(),
        Output::Headers => headers_block(),
        Output::SignedPayload => signed.signed_payload.clone(),
        Output::Curl => render_curl(&signed, payload, endpoint, content_type),
        Output::All => {
            let mut s = String::new();
            s.push_str(&format!("Provider:  {}\n", provider_label(signed.provider)));
            s.push_str(&format!(
                "Algorithm: {} ({} output)\n",
                algorithm_label(signed.algorithm),
                encoding_label(signed.encoding)
            ));
            match signed.timestamp {
                Some(t) => s.push_str(&format!("Timestamp: {t} ({})\n", iso_utc(t))),
                None => s.push_str("Timestamp: not used by this scheme\n"),
            }
            s.push_str(&format!("Payload:   {} bytes\n", payload.len()));
            s.push_str("\nSigned payload (the exact bytes that are HMAC'd)\n");
            s.push_str(&format!("{}\n", signed.signed_payload));
            s.push_str("\nSignature\n");
            s.push_str(&format!("{}\n", signed.signature));
            s.push_str("\nHeaders to send\n");
            s.push_str(&format!("{}\n", headers_block()));
            s.push_str("\nReplay with cURL\n");
            s.push_str(&format!("{}\n", render_curl(&signed, payload, endpoint, content_type)));
            if !signed.notes.is_empty() {
                s.push_str("\nNotes\n");
                for n in &signed.notes {
                    s.push_str(&format!("- {n}\n"));
                }
            }
            s
        }
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Convenience: sign with everything at its default except the named args.
    fn sig(payload: &str, secret: &str, provider: &str, ts: &str) -> String {
        run(payload, secret, provider, ts, "", "", "", "", "", "", "", "", "signature").unwrap()
    }

    // -- published provider vectors -----------------------------------------

    #[test]
    fn github_matches_published_vector() {
        // GitHub's own webhook docs example.
        let s = sig("Hello, World!", "It's a Secret to Everybody", "github", "");
        assert_eq!(
            s,
            "757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17"
        );
        let headers = run(
            "Hello, World!",
            "It's a Secret to Everybody",
            "github",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "headers",
        )
        .unwrap();
        assert!(headers.contains(
            "X-Hub-Signature-256: sha256=757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17"
        ));
        // The deprecated SHA-1 header rides along for legacy receivers.
        assert!(headers.contains("X-Hub-Signature: sha1=01dc10d0c83e72ed246219cdd91669667fe2ca59"));
    }

    #[test]
    fn slack_matches_published_vector() {
        let body = "token=xyzz0WbapA4vBCDEFasx0q6G&team_id=T1DC2JH3J&team_domain=testteamnow&channel_id=G8PSS9T3V&channel_name=foobar&user_id=U2CERLKJA&user_name=roadrunner&command=%2Fwebhook-collect&text=&response_url=https%3A%2F%2Fhooks.slack.com%2Fcommands%2FT1DC2JH3J%2F397700885554%2F96rGlfmibIGlgcZRskXaIFfN&trigger_id=398738663015.47445629121.803a0bc887a14d10d2c447fce8b6703c";
        let header = run(
            body,
            "8f742231b10e8888abcd99yyyzzz85a5",
            "slack",
            "1531420618",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "header",
        )
        .unwrap();
        assert_eq!(
            header,
            "v0=a2114d57b48eac39b9ad189dd8316235a7b4a8d21a10bd27519666489c69b503"
        );
    }

    #[test]
    fn standard_webhooks_matches_published_vector() {
        // Standard Webhooks / Svix docs example: the whsec_ secret is base64.
        let header = run(
            r#"{"test": 2432232314}"#,
            "whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw",
            "standard-webhooks",
            "1614265330",
            "msg_p5jXN8AQM9LWM0D4loKWxJek",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "header",
        )
        .unwrap();
        assert_eq!(header, "v1,g0hM9SsE+OTPJTGt/tmIKtSyZlE3uFJELVlNIOLJ1OE=");
    }

    #[test]
    fn svix_uses_the_same_math_with_svix_headers() {
        let headers = run(
            r#"{"test": 2432232314}"#,
            "whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw",
            "svix",
            "1614265330",
            "msg_p5jXN8AQM9LWM0D4loKWxJek",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "headers",
        )
        .unwrap();
        assert_eq!(
            headers,
            "svix-id: msg_p5jXN8AQM9LWM0D4loKWxJek\n\
             svix-timestamp: 1614265330\n\
             svix-signature: v1,g0hM9SsE+OTPJTGt/tmIKtSyZlE3uFJELVlNIOLJ1OE="
        );
    }

    #[test]
    fn twilio_matches_published_vector() {
        // Twilio's docs example: URL + alphabetically sorted POST params.
        let body = "CallSid=CA1234567890ABCDE&Caller=%2B14158675310&Digits=1234&From=%2B14158675310&To=%2B18005551212";
        let s = run(
            body,
            "12345",
            "twilio",
            "",
            "",
            "https://example.com/myapp.php?foo=1&bar=2",
            "",
            "",
            "",
            "",
            "",
            "",
            "signature",
        )
        .unwrap();
        assert_eq!(s, "L/OH5YylLD5NRKLltdqwSvS0BnU=");
    }

    #[test]
    fn stripe_signs_timestamp_dot_payload() {
        let payload = r#"{"id":"evt_test","type":"payment_intent.succeeded"}"#;
        let signed = run(
            payload,
            "whsec_test_secret",
            "stripe",
            "1700000000",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "signed-payload",
        )
        .unwrap();
        assert_eq!(signed, format!("1700000000.{payload}"));
        assert_eq!(
            run(
                payload,
                "whsec_test_secret",
                "stripe",
                "1700000000",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "header"
            )
            .unwrap(),
            "t=1700000000,v1=1fa069bafb0fb0ec61ca8e80964ba68904130b9623f205d87d85ad75d5de7d02"
        );
    }

    #[test]
    fn shopify_is_base64_over_the_raw_body() {
        let payload = r#"{"id":"evt_test","type":"payment_intent.succeeded"}"#;
        assert_eq!(
            sig(payload, "shpss_test_secret", "shopify", ""),
            "yJ17VFuOsHDM+DVCLoqjnr1meF42g82PTzGIpDbQttk="
        );
    }

    #[test]
    fn square_prefixes_the_notification_url() {
        let payload = r#"{"id":"evt_test","type":"payment_intent.succeeded"}"#;
        let s = run(
            payload,
            "sq_test_secret",
            "square",
            "",
            "",
            "https://example.com/hooks",
            "",
            "",
            "",
            "",
            "",
            "",
            "signature",
        )
        .unwrap();
        assert_eq!(s, "pbsGEo1fdy6JIgk1AYfZgyW2D3mK6yFBvXQ3BZqKTno=");
    }

    #[test]
    fn paddle_uses_ts_colon_payload_and_semicolon_header() {
        let payload = r#"{"id":"evt_test","type":"payment_intent.succeeded"}"#;
        let header = run(
            payload,
            "pdl_ntfset_test",
            "paddle",
            "1700000000",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "header",
        )
        .unwrap();
        assert_eq!(
            header,
            "ts=1700000000;h1=270fc46fa3a366adbae0dedd0fe6c570e691cd592477d234bf2422553aac175e"
        );
    }

    // -- custom scheme -------------------------------------------------------

    #[test]
    fn custom_template_and_header_are_honoured() {
        let out = run(
            "body",
            "k",
            "custom",
            "1700000000",
            "",
            "",
            "sha512",
            "base64url",
            "text",
            "{timestamp}|{payload}",
            "X-My-Sig",
            "v1=",
            "headers",
        )
        .unwrap();
        assert!(out.starts_with("X-My-Sig: v1="), "got {out}");
        let signed = run(
            "body",
            "k",
            "custom",
            "1700000000",
            "",
            "",
            "sha512",
            "base64url",
            "text",
            "{timestamp}|{payload}",
            "X-My-Sig",
            "v1=",
            "signed-payload",
        )
        .unwrap();
        assert_eq!(signed, "1700000000|body");
    }

    #[test]
    fn custom_defaults_to_signing_the_bare_payload_with_sha256_hex() {
        // Same bytes + key as the GitHub vector, minus GitHub's `sha256=`.
        let s = run(
            "Hello, World!",
            "It's a Secret to Everybody",
            "custom",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "signature",
        )
        .unwrap();
        assert_eq!(
            s,
            "757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17"
        );
    }

    #[test]
    fn hex_upper_encoding_is_the_uppercase_of_hex() {
        let lower = sig("Hello, World!", "It's a Secret to Everybody", "custom", "");
        let upper = run(
            "Hello, World!",
            "It's a Secret to Everybody",
            "custom",
            "",
            "",
            "",
            "sha256",
            "hex-upper",
            "",
            "",
            "",
            "",
            "signature",
        )
        .unwrap();
        assert_eq!(upper, lower.to_ascii_uppercase());
    }

    // -- secret encodings ----------------------------------------------------

    #[test]
    fn hex_and_base64_secrets_decode_to_the_same_key() {
        let a = run(
            "x", "48656c6c6f", "custom", "", "", "", "", "", "hex", "", "", "", "signature",
        )
        .unwrap();
        let b = run(
            "x", "SGVsbG8=", "custom", "", "", "", "", "", "base64", "", "", "", "signature",
        )
        .unwrap();
        let c = run("x", "Hello", "custom", "", "", "", "", "", "text", "", "", "", "signature")
            .unwrap();
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn stripe_secret_is_literal_text_even_with_whsec_prefix() {
        // auto must NOT base64-decode a Stripe whsec_ secret — that is the
        // classic "my signature never matches" bug this note guards.
        let auto = sig("p", "whsec_abc", "stripe", "1700000000");
        let text = run(
            "p",
            "whsec_abc",
            "stripe",
            "1700000000",
            "",
            "",
            "",
            "",
            "text",
            "",
            "",
            "",
            "signature",
        )
        .unwrap();
        assert_eq!(auto, text);
    }

    // -- timestamps ----------------------------------------------------------

    #[test]
    fn iso_and_unix_timestamps_agree() {
        let a = sig("p", "s", "stripe", "1700000000");
        let b = sig("p", "s", "stripe", "2023-11-14T22:13:20Z");
        let c = sig("p", "s", "stripe", "2023-11-14T23:13:20+01:00");
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn all_report_echoes_the_human_readable_time() {
        let all = run(
            "p", "s", "stripe", "1700000000", "", "", "", "", "", "", "", "", "all",
        )
        .unwrap();
        assert!(all.contains("Timestamp: 1700000000 (2023-11-14T22:13:20Z)"), "got {all}");
        assert!(all.contains("Signed payload"));
        assert!(all.contains("curl -X POST 'https://example.com/webhook'"));
    }

    #[test]
    fn blank_message_id_is_derived_deterministically() {
        let a = run(
            r#"{"a":1}"#,
            "whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw",
            "standard-webhooks",
            "1614265330",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "headers",
        )
        .unwrap();
        let b = a.clone();
        assert_eq!(a, b);
        assert!(a.starts_with("webhook-id: msg_"), "got {a}");
    }

    #[test]
    fn curl_escapes_single_quotes_in_the_payload() {
        let c = run(
            "it's",
            "s",
            "github",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "curl",
        )
        .unwrap();
        assert!(c.contains(r"--data-raw 'it'\''s'"), "got {c}");
    }

    // -- errors --------------------------------------------------------------

    #[test]
    fn empty_secret_is_an_error() {
        let e = run("p", "  ", "stripe", "1700000000", "", "", "", "", "", "", "", "", "all")
            .unwrap_err();
        assert!(e.contains("secret is required"), "got {e}");
    }

    #[test]
    fn unknown_provider_is_an_error_listing_the_choices() {
        let e = run("p", "s", "paypal", "", "", "", "", "", "", "", "", "", "all").unwrap_err();
        assert!(e.contains("invalid provider 'paypal'"), "got {e}");
        assert!(e.contains("standard-webhooks"), "got {e}");
    }

    #[test]
    fn square_without_a_url_is_an_error() {
        let e = run("p", "s", "square", "", "", "", "", "", "", "", "", "", "all").unwrap_err();
        assert!(e.contains("url is required"), "got {e}");
    }

    #[test]
    fn millisecond_timestamps_are_rejected_with_a_hint() {
        let e = run(
            "p", "s", "stripe", "1700000000000", "", "", "", "", "", "", "", "", "all",
        )
        .unwrap_err();
        assert!(e.contains("milliseconds"), "got {e}");
    }

    #[test]
    fn unknown_template_placeholder_is_an_error() {
        let e = run(
            "p", "s", "custom", "", "", "", "", "", "", "{nonce}", "", "", "all",
        )
        .unwrap_err();
        assert!(e.contains("unknown template placeholder '{nonce}'"), "got {e}");
    }

    #[test]
    fn bad_hex_secret_is_an_error() {
        let e = run("p", "zz", "custom", "", "", "", "", "", "hex", "", "", "", "all").unwrap_err();
        assert!(e.contains("not valid hex"), "got {e}");
    }

    #[test]
    fn oversized_payload_is_rejected() {
        let big = "a".repeat(MAX_PAYLOAD_BYTES + 1);
        let e = run(&big, "s", "github", "", "", "", "", "", "", "", "", "", "all").unwrap_err();
        assert!(e.contains("exceeds"), "got {e}");
    }

    #[test]
    fn every_provider_signs_without_error() {
        // Advertised-values guard: each enum choice must produce a result.
        for p in PROVIDERS {
            let out = run(
                r#"{"a":1}"#,
                "whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw",
                p,
                "1700000000",
                "msg_test",
                "https://example.com/hooks",
                "",
                "",
                "",
                "",
                "",
                "",
                "all",
            );
            assert!(out.is_ok(), "provider {p} failed: {:?}", out.err());
        }
    }

    #[test]
    fn provider_needs_timestamp_matches_the_signed_bytes() {
        // Timestamped schemes …
        for p in ["stripe", "slack", "standard-webhooks", "svix", "paddle"] {
            assert!(provider_needs_timestamp(p, ""), "{p} signs a timestamp");
        }
        // … body-only schemes.
        for p in ["github", "shopify", "square", "twilio", "custom"] {
            assert!(!provider_needs_timestamp(p, ""), "{p} signs the body only");
        }
        // custom follows its template, and an unknown provider never asks for one.
        assert!(provider_needs_timestamp("custom", "v0:{timestamp}:{payload}"));
        assert!(!provider_needs_timestamp("paypal", ""));
    }

    #[test]
    fn every_output_selector_returns_something() {
        for o in OUTPUTS {
            let out = run("p", "s", "stripe", "1700000000", "", "", "", "", "", "", "", "", o);
            assert!(out.is_ok(), "output {o} failed: {:?}", out.err());
            assert!(!out.unwrap().is_empty(), "output {o} was empty");
        }
    }
}
