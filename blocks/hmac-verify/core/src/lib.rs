//! hmac-verify core — verify a supplied HMAC tag against a message and secret
//! key, using a constant-time comparison. Pure compute, shared by the chat
//! skill block and the web page. No wafer/wasm-bindgen deps.
//!
//! HMAC (RFC 2104) proves both integrity and authenticity: only a holder of the
//! secret key can produce the tag. To verify a signature you recompute the HMAC
//! over the same message and key and compare it to the tag you were given. That
//! comparison MUST be timing-safe — a naive `==` that returns on the first
//! differing byte leaks how many leading bytes were correct and can let an
//! attacker forge a tag byte-by-byte. This tool recomputes the tag and compares
//! it with a length-independent, non-short-circuiting compare, then reports
//! MATCH / MISMATCH.
//!
//! The message and key can each be read as UTF-8 text (default) or decoded from
//! hex / base64 first; the expected tag can be given as hex or base64 (or
//! auto-detected). All hashers are pure-Rust (RustCrypto) so the tool runs on
//! every backend, including the chat Service Worker.

use base64::Engine;
use hmac::{Hmac, Mac};

/// Which underlying hash algorithm the HMAC uses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Algorithm {
    Md5,
    Sha1,
    Sha224,
    Sha256,
    Sha384,
    Sha512,
    Sha3_256,
    Sha3_512,
}

impl Algorithm {
    /// Canonical lowercase identifier (matches the descriptor enum values).
    pub fn name(self) -> &'static str {
        match self {
            Algorithm::Md5 => "md5",
            Algorithm::Sha1 => "sha1",
            Algorithm::Sha224 => "sha224",
            Algorithm::Sha256 => "sha256",
            Algorithm::Sha384 => "sha384",
            Algorithm::Sha512 => "sha512",
            Algorithm::Sha3_256 => "sha3-256",
            Algorithm::Sha3_512 => "sha3-512",
        }
    }
}

/// How to interpret an input string (message or key) before use.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputEncoding {
    /// Use the UTF-8 bytes of the text as-is (default).
    Text,
    /// Decode the text from hexadecimal first, then use the raw bytes.
    Hex,
    /// Decode the text from base64 (standard alphabet) first, then use the bytes.
    Base64,
}

/// How to interpret the expected tag string.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TagEncoding {
    /// Try hex first, then base64 (default) — accepts whichever a provider sent.
    Auto,
    /// Decode from hexadecimal.
    Hex,
    /// Decode from standard base64.
    Base64,
}

/// The canonical list of supported algorithm identifiers, in menu order. Kept
/// in one place so the descriptor enum, manifest, and page stay in sync.
pub const ALGORITHMS: &[&str] = &[
    "md5", "sha1", "sha224", "sha256", "sha384", "sha512", "sha3-256", "sha3-512",
];

fn parse_algorithm(s: &str) -> Result<Algorithm, String> {
    // Normalize: lowercase, and strip '-'/'_' so "sha3-256", "sha3_256",
    // "SHA-256" etc. all parse to the same algorithm.
    let lower = s.trim().to_ascii_lowercase();
    let canon: String = lower.chars().filter(|c| *c != '-' && *c != '_').collect();
    let alg = match canon.as_str() {
        "" | "sha256" => Algorithm::Sha256, // sha256 is the default
        "md5" => Algorithm::Md5,
        "sha1" => Algorithm::Sha1,
        "sha224" => Algorithm::Sha224,
        "sha384" => Algorithm::Sha384,
        "sha512" => Algorithm::Sha512,
        "sha3256" => Algorithm::Sha3_256,
        "sha3512" => Algorithm::Sha3_512,
        other => {
            return Err(format!(
                "invalid algorithm '{other}': expected one of {}",
                ALGORITHMS.join(", ")
            ))
        }
    };
    Ok(alg)
}

fn parse_input_encoding(s: &str, field: &str) -> Result<InputEncoding, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "text" | "utf8" | "utf-8" => Ok(InputEncoding::Text),
        "hex" => Ok(InputEncoding::Hex),
        "base64" | "b64" => Ok(InputEncoding::Base64),
        other => Err(format!(
            "invalid {field} '{other}': expected 'text', 'hex', or 'base64'"
        )),
    }
}

fn parse_tag_encoding(s: &str) -> Result<TagEncoding, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(TagEncoding::Auto),
        "hex" => Ok(TagEncoding::Hex),
        "base64" | "b64" => Ok(TagEncoding::Base64),
        other => Err(format!(
            "invalid expected_encoding '{other}': expected 'auto', 'hex', or 'base64'"
        )),
    }
}

/// Decode `text` into raw bytes per `encoding`. `field` names the value for
/// error messages (e.g. "message" / "key").
fn decode_input(text: &str, encoding: InputEncoding, field: &str) -> Result<Vec<u8>, String> {
    match encoding {
        InputEncoding::Text => Ok(text.as_bytes().to_vec()),
        InputEncoding::Hex => {
            // Tolerate an optional 0x prefix and internal whitespace (matches
            // hmac-generate / hash-text so the hash family accepts the same input).
            let cleaned: String = text.split_whitespace().collect();
            let cleaned = cleaned
                .strip_prefix("0x")
                .or_else(|| cleaned.strip_prefix("0X"))
                .unwrap_or(&cleaned);
            hex::decode(cleaned).map_err(|e| format!("{field} is not valid hex: {e}"))
        }
        InputEncoding::Base64 => {
            let cleaned: String = text.split_whitespace().collect();
            base64::engine::general_purpose::STANDARD
                .decode(cleaned.as_bytes())
                .map_err(|e| format!("{field} is not valid base64: {e}"))
        }
    }
}

/// Decode the expected tag string into raw bytes. Tolerates an optional `0x`
/// hex prefix and common webhook signature prefixes (`sha256=`, `sha1=`, `v1=`)
/// plus surrounding whitespace, so a header value can be pasted directly.
fn decode_expected(expected: &str, encoding: TagEncoding) -> Result<Vec<u8>, String> {
    let trimmed = expected.trim();
    if trimmed.is_empty() {
        return Err("expected tag is empty".to_string());
    }
    // Strip a leading `algo=`/`v1=` label (GitHub `sha256=…`, Stripe `v1=…`).
    let stripped = trimmed
        .split_once('=')
        .filter(|(label, value)| {
            matches!(
                label.to_ascii_lowercase().as_str(),
                "sha1"
                    | "sha224"
                    | "sha256"
                    | "sha384"
                    | "sha512"
                    | "sha3-256"
                    | "sha3-512"
                    | "md5"
                    | "v1"
            ) && !value.is_empty()
        })
        .map(|(_, v)| v)
        .unwrap_or(trimmed);
    let candidate: String = stripped.split_whitespace().collect();

    let try_hex = |s: &str| {
        let s = s
            .strip_prefix("0x")
            .or_else(|| s.strip_prefix("0X"))
            .unwrap_or(s);
        hex::decode(s).ok()
    };
    let try_b64 = |s: &str| {
        base64::engine::general_purpose::STANDARD
            .decode(s.as_bytes())
            .ok()
    };

    match encoding {
        TagEncoding::Hex => {
            let s = candidate
                .strip_prefix("0x")
                .or_else(|| candidate.strip_prefix("0X"))
                .unwrap_or(&candidate);
            hex::decode(s).map_err(|e| format!("expected tag is not valid hex: {e}"))
        }
        TagEncoding::Base64 => {
            try_b64(&candidate).ok_or_else(|| "expected tag is not valid base64".to_string())
        }
        TagEncoding::Auto => try_hex(&candidate)
            .or_else(|| try_b64(&candidate))
            .ok_or_else(|| {
                "expected tag is neither valid hex nor valid base64 (set expected_encoding \
                 explicitly)"
                    .to_string()
            }),
    }
}

/// Compute the raw HMAC tag of `message` keyed by `key` with `alg`.
fn hmac_bytes(key: &[u8], message: &[u8], alg: Algorithm) -> Vec<u8> {
    use sha2::{Sha224, Sha256, Sha384, Sha512};
    use sha3::{Sha3_256, Sha3_512};
    fn mac<D>(key: &[u8], message: &[u8]) -> Vec<u8>
    where
        D: digest::core_api::CoreProxy,
        D::Core: Sync
            + Send
            + Clone
            + digest::core_api::BufferKindUser<BufferKind = digest::block_buffer::Eager>
            + digest::core_api::FixedOutputCore
            + digest::HashMarker
            + Default,
        <D::Core as digest::core_api::BlockSizeUser>::BlockSize:
            digest::typenum::IsLess<digest::consts::U256>,
        digest::typenum::Le<
            <D::Core as digest::core_api::BlockSizeUser>::BlockSize,
            digest::consts::U256,
        >: digest::typenum::NonZero,
    {
        // HMAC accepts a key of any length (it is internally padded/hashed to the
        // block size), so new_from_slice never fails here.
        let mut m = Hmac::<D>::new_from_slice(key).expect("hmac accepts any key length");
        m.update(message);
        m.finalize().into_bytes().to_vec()
    }
    match alg {
        Algorithm::Md5 => mac::<md5::Md5>(key, message),
        Algorithm::Sha1 => mac::<sha1::Sha1>(key, message),
        Algorithm::Sha224 => mac::<Sha224>(key, message),
        Algorithm::Sha256 => mac::<Sha256>(key, message),
        Algorithm::Sha384 => mac::<Sha384>(key, message),
        Algorithm::Sha512 => mac::<Sha512>(key, message),
        Algorithm::Sha3_256 => mac::<Sha3_256>(key, message),
        Algorithm::Sha3_512 => mac::<Sha3_512>(key, message),
    }
}

/// Length-independent byte comparison that does not short-circuit on the first
/// differing byte (avoids leaking match progress via timing). This is the
/// timing-safe compare at the heart of HMAC verification.
fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// The outcome of verifying an HMAC tag.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Verification {
    /// Whether the recomputed tag matches the supplied one (constant-time).
    pub matched: bool,
    /// The algorithm used for the check.
    pub algorithm: Algorithm,
    /// The tag recomputed from the message + key, lowercase hex.
    pub computed_hex: String,
    /// The supplied expected tag, normalized to lowercase hex.
    pub expected_hex: String,
    /// A one-line human summary.
    pub summary: String,
}

/// Verify that the HMAC of `message` (per `message_encoding`) keyed by `key`
/// (per `key_encoding`) with `algorithm` equals the `expected` tag (decoded per
/// `expected_encoding`). The comparison is timing-safe.
///
/// Defaults (blank strings): algorithm=sha256, message_encoding=text,
/// key_encoding=text, expected_encoding=auto.
pub fn verify(
    message: &str,
    key: &str,
    expected: &str,
    algorithm: &str,
    message_encoding: &str,
    key_encoding: &str,
    expected_encoding: &str,
) -> Result<Verification, String> {
    let alg = parse_algorithm(algorithm)?;
    let msg_enc = parse_input_encoding(message_encoding, "message_encoding")?;
    let key_enc = parse_input_encoding(key_encoding, "key_encoding")?;
    let tag_enc = parse_tag_encoding(expected_encoding)?;

    let msg_bytes = decode_input(message, msg_enc, "message")?;
    let key_bytes = decode_input(key, key_enc, "key")?;
    let expected_bytes = decode_expected(expected, tag_enc)?;

    let computed = hmac_bytes(&key_bytes, &msg_bytes, alg);
    let matched = constant_eq(&computed, &expected_bytes);

    let summary = if matched {
        format!(
            "MATCH — the tag is a valid HMAC-{} of the message under this key.",
            alg.name().to_uppercase()
        )
    } else if computed.len() != expected_bytes.len() {
        format!(
            "MISMATCH — the tag does NOT verify. The expected tag is {} bytes but HMAC-{} \
             produces a {}-byte tag; check the algorithm.",
            expected_bytes.len(),
            alg.name().to_uppercase(),
            computed.len()
        )
    } else {
        format!(
            "MISMATCH — the tag does NOT verify against this message and key (HMAC-{}).",
            alg.name().to_uppercase()
        )
    };

    Ok(Verification {
        matched,
        algorithm: alg,
        computed_hex: hex::encode(&computed),
        expected_hex: hex::encode(&expected_bytes),
        summary,
    })
}

/// Render a [`verify`] result as a multi-line text report (the CLI / chat /
/// page string surface).
#[allow(clippy::too_many_arguments)]
pub fn verify_report(
    message: &str,
    key: &str,
    expected: &str,
    algorithm: &str,
    message_encoding: &str,
    key_encoding: &str,
    expected_encoding: &str,
) -> Result<String, String> {
    let v = verify(
        message,
        key,
        expected,
        algorithm,
        message_encoding,
        key_encoding,
        expected_encoding,
    )?;
    Ok(format!(
        "{}\nstatus:    {}\nalgorithm: {}\nexpected:  {}\ncomputed:  {}",
        v.summary,
        if v.matched { "MATCH" } else { "MISMATCH" },
        v.algorithm.name(),
        v.expected_hex,
        v.computed_hex,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 4231 test case 2: key = "Jefe", data = "what do ya want for nothing?".
    const KEY: &str = "Jefe";
    const MSG: &str = "what do ya want for nothing?";
    // HMAC-SHA256(Jefe, MSG).
    const TAG_SHA256: &str = "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843";

    #[test]
    fn matching_tag_verifies() {
        let v = verify(MSG, KEY, TAG_SHA256, "sha256", "", "", "").unwrap();
        assert!(v.matched, "{}", v.summary);
        assert!(v.summary.starts_with("MATCH"));
        assert_eq!(v.computed_hex, TAG_SHA256);
    }

    #[test]
    fn wrong_tag_does_not_verify() {
        // Flip the last hex nibble.
        let bad = "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3844";
        let v = verify(MSG, KEY, bad, "sha256", "", "", "").unwrap();
        assert!(!v.matched, "{}", v.summary);
        assert!(v.summary.starts_with("MISMATCH"));
    }

    #[test]
    fn wrong_key_does_not_verify() {
        let v = verify(MSG, "wrong-key", TAG_SHA256, "sha256", "", "", "").unwrap();
        assert!(!v.matched);
    }

    #[test]
    fn default_algorithm_is_sha256() {
        // Blank algorithm behaves like sha256.
        let v = verify(MSG, KEY, TAG_SHA256, "", "", "", "").unwrap();
        assert!(v.matched);
        assert_eq!(v.algorithm, Algorithm::Sha256);
    }

    #[test]
    fn uppercase_hex_tag_verifies() {
        let v = verify(MSG, KEY, &TAG_SHA256.to_uppercase(), "sha256", "", "", "").unwrap();
        assert!(v.matched);
    }

    #[test]
    fn base64_tag_auto_detected() {
        // HMAC-SHA256(Jefe, MSG) in base64.
        let b64 = "W9zBRr9gdU5qBCQmCJV1x1oAPwidJzmDnexYuWTsOEM=";
        let v = verify(MSG, KEY, b64, "sha256", "", "", "auto").unwrap();
        assert!(v.matched, "{}", v.summary);
    }

    #[test]
    fn base64_tag_explicit_encoding() {
        let b64 = "W9zBRr9gdU5qBCQmCJV1x1oAPwidJzmDnexYuWTsOEM=";
        let v = verify(MSG, KEY, b64, "sha256", "", "", "base64").unwrap();
        assert!(v.matched);
    }

    #[test]
    fn github_signature_prefix_stripped() {
        let sig = format!("sha256={TAG_SHA256}");
        let v = verify(MSG, KEY, &sig, "sha256", "", "", "").unwrap();
        assert!(v.matched, "{}", v.summary);
    }

    #[test]
    fn hex_prefix_0x_tolerated() {
        let sig = format!("0x{TAG_SHA256}");
        let v = verify(MSG, KEY, &sig, "sha256", "", "", "hex").unwrap();
        assert!(v.matched);
    }

    #[test]
    fn sha1_legacy_tag_verifies() {
        // RFC 2202 HMAC-SHA1 test case 2.
        let tag = "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79";
        let v = verify(MSG, KEY, tag, "sha1", "", "", "").unwrap();
        assert!(v.matched, "{}", v.summary);
    }

    #[test]
    fn sha512_tag_verifies() {
        let tag = "164b7a7bfcf819e2e395fbe73b56e0a387bd64222e831fd610270cd7ea2505549758bf75c05a994a6d034f65f8f0e6fdcaeab1a34d4a6b4b636e070a38bce737";
        let v = verify(MSG, KEY, tag, "sha512", "", "", "").unwrap();
        assert!(v.matched);
    }

    #[test]
    fn hex_key_matches_text_key() {
        // "Jefe" as hex is 4a656665.
        let v = verify(MSG, "4a656665", TAG_SHA256, "sha256", "", "hex", "").unwrap();
        assert!(v.matched);
    }

    #[test]
    fn wrong_algorithm_reports_length_hint() {
        // A SHA-256 tag checked as SHA-512 mismatches on length.
        let v = verify(MSG, KEY, TAG_SHA256, "sha512", "", "", "").unwrap();
        assert!(!v.matched);
        assert!(v.summary.contains("bytes"), "{}", v.summary);
    }

    #[test]
    fn invalid_algorithm_errors() {
        let e = verify(MSG, KEY, TAG_SHA256, "sha999", "", "", "").unwrap_err();
        assert!(e.contains("invalid algorithm"), "{e}");
    }

    #[test]
    fn invalid_expected_encoding_errors() {
        let e = verify(MSG, KEY, TAG_SHA256, "sha256", "", "", "rot13").unwrap_err();
        assert!(e.contains("invalid expected_encoding"), "{e}");
    }

    #[test]
    fn empty_expected_errors() {
        let e = verify(MSG, KEY, "", "sha256", "", "", "").unwrap_err();
        assert!(e.contains("empty"), "{e}");
    }

    #[test]
    fn undecodable_expected_errors() {
        // 'zz' is not hex; with a trailing '!' it isn't base64 either.
        let e = verify(MSG, KEY, "zz!!", "sha256", "", "", "hex").unwrap_err();
        assert!(e.contains("not valid hex"), "{e}");
    }

    #[test]
    fn report_shape() {
        let out = verify_report(MSG, KEY, TAG_SHA256, "sha256", "", "", "").unwrap();
        assert!(out.contains("status:    MATCH"));
        assert!(out.contains("algorithm: sha256"));
        assert!(out.contains(&format!("computed:  {TAG_SHA256}")));
    }
}
