//! gizza-ai/jwt-verify core — decode a JSON Web Token (JWT/JWS compact
//! serialization) and verify its signature against a shared HMAC secret
//! (HS256/384/512), an RSA public key (RS256/384/512, RSASSA-PKCS1-v1_5) or an
//! EC public key (ES256/384, ECDSA P-256/P-384), then validate the standard
//! time + identity claims (`exp`, `nbf`, `iat`, `iss`, `aud`). Pure-Rust; no
//! wafer / wasm-bindgen deps. The current time is supplied by the caller so the
//! function stays deterministic across surfaces (chat/CLI/page).

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine;
use serde_json::{json, Value};

/// Supported JWS algorithms (signature verification).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alg {
    Hs256,
    Hs384,
    Hs512,
    Rs256,
    Rs384,
    Rs512,
    Es256,
    Es384,
}

impl Alg {
    pub fn parse(s: &str) -> Result<Alg, String> {
        match s.trim().to_ascii_uppercase().replace(['-', '_', ' '], "").as_str() {
            "HS256" => Ok(Alg::Hs256),
            "HS384" => Ok(Alg::Hs384),
            "HS512" => Ok(Alg::Hs512),
            "RS256" => Ok(Alg::Rs256),
            "RS384" => Ok(Alg::Rs384),
            "RS512" => Ok(Alg::Rs512),
            "ES256" => Ok(Alg::Es256),
            "ES384" => Ok(Alg::Es384),
            other => Err(format!(
                "unsupported algorithm '{other}' (HS256/384/512, RS256/384/512, ES256/384)"
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Alg::Hs256 => "HS256",
            Alg::Hs384 => "HS384",
            Alg::Hs512 => "HS512",
            Alg::Rs256 => "RS256",
            Alg::Rs384 => "RS384",
            Alg::Rs512 => "RS512",
            Alg::Es256 => "ES256",
            Alg::Es384 => "ES384",
        }
    }

    fn is_hmac(self) -> bool {
        matches!(self, Alg::Hs256 | Alg::Hs384 | Alg::Hs512)
    }
}

/// Options for verification. All claim checks are optional; the signature check
/// is always performed. `now` is "seconds since the Unix epoch" supplied by the
/// caller's clock. `leeway` is a tolerance (in seconds) applied to `exp`/`nbf`
/// to absorb small clock skew.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// If set, the token's `alg` header must equal this algorithm (defends
    /// against `alg` confusion / downgrade attacks). When `None`, the alg is
    /// taken from the header (but `none` is always rejected).
    pub expected_alg: Option<Alg>,
    /// If set, the `iss` claim must equal this value.
    pub expected_iss: Option<String>,
    /// If set, the `aud` claim must contain (or equal) this value.
    pub expected_aud: Option<String>,
    /// Current time (seconds since Unix epoch). Used for `exp`/`nbf` checks.
    pub now: i64,
    /// Clock-skew tolerance in seconds for `exp`/`nbf`.
    pub leeway: i64,
}

/// One validation check that was performed, for transparent reporting.
#[derive(Debug, Clone)]
pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

/// The outcome of verifying a token.
#[derive(Debug, Clone)]
pub struct VerifyResult {
    pub valid: bool,
    pub algorithm: String,
    pub header: Value,
    pub payload: Value,
    pub checks: Vec<Check>,
    /// The first failing check's message, if any (the headline reason invalid).
    pub error: Option<String>,
}

impl VerifyResult {
    /// Render as a serde_json object (used by the chat/CLI surface).
    pub fn to_json(&self) -> Value {
        json!({
            "valid": self.valid,
            "algorithm": self.algorithm,
            "error": self.error,
            "header": self.header,
            "payload": self.payload,
            "checks": self.checks.iter().map(|c| json!({
                "check": c.name,
                "ok": c.ok,
                "detail": c.detail,
            })).collect::<Vec<_>>(),
        })
    }
}

fn b64url_decode(label: &str, s: &str) -> Result<Vec<u8>, String> {
    B64URL
        .decode(s.trim())
        .map_err(|e| format!("{label} is not valid base64url: {e}"))
}

/// Split a compact JWT into its three base64url parts.
fn split_compact(token: &str) -> Result<(&str, &str, &str), String> {
    let t = token.trim();
    let parts: Vec<&str> = t.split('.').collect();
    if parts.len() != 3 {
        return Err(format!(
            "not a compact JWS: expected 3 dot-separated parts, found {}",
            parts.len()
        ));
    }
    if parts.iter().any(|p| p.is_empty()) {
        // An empty signature is the `alg: none` (unsecured) form — we reject it.
        return Err("token has an empty segment (unsecured 'alg:none' tokens are not accepted)".into());
    }
    Ok((parts[0], parts[1], parts[2]))
}

fn hmac_verify(key: &[u8], data: &[u8], sig: &[u8], alg: Alg) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::{Sha256, Sha384, Sha512};
    match alg {
        Alg::Hs256 => {
            let mut m = match <Hmac<Sha256>>::new_from_slice(key) {
                Ok(m) => m,
                Err(_) => return false,
            };
            m.update(data);
            m.verify_slice(sig).is_ok()
        }
        Alg::Hs384 => {
            let mut m = match <Hmac<Sha384>>::new_from_slice(key) {
                Ok(m) => m,
                Err(_) => return false,
            };
            m.update(data);
            m.verify_slice(sig).is_ok()
        }
        Alg::Hs512 => {
            let mut m = match <Hmac<Sha512>>::new_from_slice(key) {
                Ok(m) => m,
                Err(_) => return false,
            };
            m.update(data);
            m.verify_slice(sig).is_ok()
        }
        _ => false,
    }
}

fn rsa_verify(key_pem: &str, data: &[u8], sig: &[u8], alg: Alg) -> Result<bool, String> {
    use rsa::pkcs1::DecodeRsaPublicKey;
    use rsa::pkcs1v15::{Signature, VerifyingKey};
    use rsa::pkcs8::DecodePublicKey;
    use rsa::signature::Verifier;
    use rsa::RsaPublicKey;
    use sha2::{Sha256, Sha384, Sha512};

    if !key_pem.contains("PUBLIC KEY") {
        return Err("no RSA public key found — paste a PEM '-----BEGIN PUBLIC KEY-----' (SPKI) or '-----BEGIN RSA PUBLIC KEY-----' (PKCS#1) block".into());
    }
    let pubk = RsaPublicKey::from_public_key_pem(key_pem)
        .or_else(|_| RsaPublicKey::from_pkcs1_pem(key_pem))
        .map_err(|e| format!("invalid RSA public key: {e}"))?;
    let sig = match alg {
        Alg::Rs256 => Signature::try_from(sig),
        Alg::Rs384 => Signature::try_from(sig),
        Alg::Rs512 => Signature::try_from(sig),
        _ => unreachable!(),
    }
    .map_err(|e| format!("malformed RSA signature: {e}"))?;
    let ok = match alg {
        Alg::Rs256 => VerifyingKey::<Sha256>::new(pubk).verify(data, &sig).is_ok(),
        Alg::Rs384 => VerifyingKey::<Sha384>::new(pubk).verify(data, &sig).is_ok(),
        Alg::Rs512 => VerifyingKey::<Sha512>::new(pubk).verify(data, &sig).is_ok(),
        _ => unreachable!(),
    };
    Ok(ok)
}

fn ecdsa_verify(key_pem: &str, data: &[u8], sig: &[u8], alg: Alg) -> Result<bool, String> {
    if !key_pem.contains("PUBLIC KEY") {
        return Err("no EC public key found — paste a PEM '-----BEGIN PUBLIC KEY-----' (SPKI) block".into());
    }
    // JWS ES* signatures are fixed-length raw r||s (IEEE P1363), NOT DER.
    match alg {
        Alg::Es256 => {
            use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
            use p256::pkcs8::DecodePublicKey;
            if sig.len() != 64 {
                return Err(format!(
                    "ES256 signature must be 64 bytes (raw r||s), got {}",
                    sig.len()
                ));
            }
            let vk = VerifyingKey::from_public_key_pem(key_pem).map_err(|e| {
                format!("invalid P-256 public key (ES256 needs a P-256/prime256v1 key): {e}")
            })?;
            let s = Signature::from_slice(sig)
                .map_err(|e| format!("malformed ES256 signature: {e}"))?;
            Ok(vk.verify(data, &s).is_ok())
        }
        Alg::Es384 => {
            use p384::ecdsa::{signature::Verifier, Signature, VerifyingKey};
            use p384::pkcs8::DecodePublicKey;
            if sig.len() != 96 {
                return Err(format!(
                    "ES384 signature must be 96 bytes (raw r||s), got {}",
                    sig.len()
                ));
            }
            let vk = VerifyingKey::from_public_key_pem(key_pem).map_err(|e| {
                format!("invalid P-384 public key (ES384 needs a P-384/secp384r1 key): {e}")
            })?;
            let s = Signature::from_slice(sig)
                .map_err(|e| format!("malformed ES384 signature: {e}"))?;
            Ok(vk.verify(data, &s).is_ok())
        }
        _ => unreachable!(),
    }
}

/// Pull an integer-valued claim (`exp`/`nbf`/`iat`), accepting JSON numbers.
fn claim_i64(payload: &Value, key: &str) -> Option<i64> {
    let v = payload.get(key)?;
    if let Some(i) = v.as_i64() {
        Some(i)
    } else {
        v.as_f64().map(|f| f as i64)
    }
}

/// Decode (without verifying the signature) the header + payload of a compact
/// JWT. Useful for inspection; the returned values are NOT trusted.
pub fn decode(token: &str) -> Result<(Value, Value), String> {
    let (h, p, _s) = split_compact(token)?;
    let header: Value = serde_json::from_slice(&b64url_decode("header", h)?)
        .map_err(|e| format!("header is not valid JSON: {e}"))?;
    let payload: Value = serde_json::from_slice(&b64url_decode("payload", p)?)
        .map_err(|e| format!("payload is not valid JSON: {e}"))?;
    Ok((header, payload))
}

/// Verify a compact JWT's signature and claims.
///
/// - `token`: the compact `header.payload.signature` string.
/// - `key`: the HMAC secret (HS*) or a PEM **public** key (RS*/ES*).
/// - `opts`: claim-validation options + the caller's clock (`now`).
///
/// Returns a [`VerifyResult`]. A malformed/undecodable token is an `Err`; a
/// well-formed token that simply fails a check returns `Ok` with `valid:false`
/// and the failing reason in `error` + `checks` (so callers can see exactly
/// what failed — signature vs expiry vs audience, etc.).
pub fn verify(token: &str, key: &str, opts: &Options) -> Result<VerifyResult, String> {
    let (h_b64, p_b64, s_b64) = split_compact(token)?;
    let header: Value = serde_json::from_slice(&b64url_decode("header", h_b64)?)
        .map_err(|e| format!("header is not valid JSON: {e}"))?;
    let payload: Value = serde_json::from_slice(&b64url_decode("payload", p_b64)?)
        .map_err(|e| format!("payload is not valid JSON: {e}"))?;
    let sig = b64url_decode("signature", s_b64)?;

    // Determine the algorithm. Reject `none`, and (if requested) reject an alg
    // that doesn't match the expectation — this is the standard mitigation for
    // algorithm-confusion attacks.
    let header_alg_str = header
        .get("alg")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "header has no 'alg' field".to_string())?;
    if header_alg_str.eq_ignore_ascii_case("none") {
        return Err("token uses 'alg: none' (unsecured) — refusing to verify".into());
    }
    let header_alg = Alg::parse(header_alg_str)?;
    if let Some(exp_alg) = opts.expected_alg {
        if exp_alg != header_alg {
            let mut checks = vec![Check {
                name: "alg",
                ok: false,
                detail: format!(
                    "token alg is {} but {} was required",
                    header_alg.name(),
                    exp_alg.name()
                ),
            }];
            push_remaining_skipped(&mut checks);
            return Ok(VerifyResult {
                valid: false,
                algorithm: header_alg.name().to_string(),
                header,
                payload,
                error: Some(checks[0].detail.clone()),
                checks,
            });
        }
    }
    let alg = header_alg;

    let signing_input = format!("{h_b64}.{p_b64}");
    let sig_ok = if alg.is_hmac() {
        if key.is_empty() {
            return Err("HMAC secret is empty — provide the shared secret".into());
        }
        hmac_verify(key.as_bytes(), signing_input.as_bytes(), &sig, alg)
    } else if matches!(alg, Alg::Rs256 | Alg::Rs384 | Alg::Rs512) {
        rsa_verify(key, signing_input.as_bytes(), &sig, alg)?
    } else {
        ecdsa_verify(key, signing_input.as_bytes(), &sig, alg)?
    };

    let mut checks: Vec<Check> = Vec::new();
    checks.push(Check {
        name: "signature",
        ok: sig_ok,
        detail: if sig_ok {
            format!("{} signature is valid", alg.name())
        } else {
            format!("{} signature does NOT match the key", alg.name())
        },
    });

    // exp — expiry (token must not be used at/after exp, minus leeway).
    if let Some(exp) = claim_i64(&payload, "exp") {
        let ok = opts.now <= exp + opts.leeway;
        checks.push(Check {
            name: "exp",
            ok,
            detail: if ok {
                format!("not expired (exp {exp}, now {})", opts.now)
            } else {
                format!("EXPIRED (exp {exp}, now {})", opts.now)
            },
        });
    }

    // nbf — not-before (token not valid until nbf, plus leeway tolerance).
    if let Some(nbf) = claim_i64(&payload, "nbf") {
        let ok = opts.now + opts.leeway >= nbf;
        checks.push(Check {
            name: "nbf",
            ok,
            detail: if ok {
                format!("active (nbf {nbf}, now {})", opts.now)
            } else {
                format!("NOT YET VALID (nbf {nbf}, now {})", opts.now)
            },
        });
    }

    // iss — issuer.
    if let Some(want) = &opts.expected_iss {
        let got = payload.get("iss").and_then(|v| v.as_str());
        let ok = got == Some(want.as_str());
        checks.push(Check {
            name: "iss",
            ok,
            detail: if ok {
                format!("issuer matches ('{want}')")
            } else {
                format!("issuer mismatch (want '{want}', got {:?})", got)
            },
        });
    }

    // aud — audience (a string or an array of strings; must contain the want).
    if let Some(want) = &opts.expected_aud {
        let ok = match payload.get("aud") {
            Some(Value::String(s)) => s == want,
            Some(Value::Array(a)) => a.iter().any(|v| v.as_str() == Some(want.as_str())),
            _ => false,
        };
        checks.push(Check {
            name: "aud",
            ok,
            detail: if ok {
                format!("audience contains '{want}'")
            } else {
                format!("audience does not contain '{want}'")
            },
        });
    }

    let valid = checks.iter().all(|c| c.ok);
    let error = checks.iter().find(|c| !c.ok).map(|c| c.detail.clone());

    Ok(VerifyResult {
        valid,
        algorithm: alg.name().to_string(),
        header,
        payload,
        checks,
        error,
    })
}

/// When `alg` enforcement fails, the signature/claim checks are skipped; record
/// that explicitly so the report is honest about what was (not) checked.
fn push_remaining_skipped(checks: &mut Vec<Check>) {
    checks.push(Check {
        name: "signature",
        ok: false,
        detail: "not checked (algorithm requirement failed)".to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // Build an HS256 token inline (we don't depend on jwt-sign here).
    fn hs256(payload: &str, secret: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        let header = r#"{"alg":"HS256","typ":"JWT"}"#;
        let si = format!(
            "{}.{}",
            B64URL.encode(header.as_bytes()),
            B64URL.encode(payload.as_bytes())
        );
        let mut m = <Hmac<Sha256>>::new_from_slice(secret.as_bytes()).unwrap();
        m.update(si.as_bytes());
        let sig = m.finalize().into_bytes();
        format!("{si}.{}", B64URL.encode(sig))
    }

    fn opts_now(now: i64) -> Options {
        Options { now, ..Default::default() }
    }

    #[test]
    fn valid_hs256_signature() {
        let t = hs256(r#"{"sub":"123","name":"Ada"}"#, "secret");
        let r = verify(&t, "secret", &opts_now(0)).unwrap();
        assert!(r.valid, "{:?}", r.error);
        assert_eq!(r.algorithm, "HS256");
        assert_eq!(r.payload["sub"], "123");
        assert!(r.checks.iter().any(|c| c.name == "signature" && c.ok));
    }

    #[test]
    fn wrong_secret_fails_signature() {
        let t = hs256(r#"{"sub":"123"}"#, "secret");
        let r = verify(&t, "WRONG", &opts_now(0)).unwrap();
        assert!(!r.valid);
        assert!(r.error.as_deref().unwrap().contains("signature"));
    }

    #[test]
    fn expired_token_fails() {
        let t = hs256(r#"{"exp":1000}"#, "k");
        let r = verify(&t, "k", &opts_now(2000)).unwrap();
        assert!(!r.valid);
        assert!(r.checks.iter().any(|c| c.name == "exp" && !c.ok));
        // still valid before expiry
        let r2 = verify(&t, "k", &opts_now(500)).unwrap();
        assert!(r2.valid);
    }

    #[test]
    fn leeway_absorbs_skew() {
        let t = hs256(r#"{"exp":1000}"#, "k");
        // now=1030 is past exp but within 60s leeway
        let r = verify(&t, "k", &Options { now: 1030, leeway: 60, ..Default::default() }).unwrap();
        assert!(r.valid, "{:?}", r.error);
    }

    #[test]
    fn nbf_not_yet_valid() {
        let t = hs256(r#"{"nbf":5000}"#, "k");
        let r = verify(&t, "k", &opts_now(1000)).unwrap();
        assert!(!r.valid);
        assert!(r.checks.iter().any(|c| c.name == "nbf" && !c.ok));
        let r2 = verify(&t, "k", &opts_now(6000)).unwrap();
        assert!(r2.valid);
    }

    #[test]
    fn issuer_and_audience() {
        let t = hs256(r#"{"iss":"me","aud":["a","b"]}"#, "k");
        let good = Options {
            now: 0,
            expected_iss: Some("me".into()),
            expected_aud: Some("b".into()),
            ..Default::default()
        };
        assert!(verify(&t, "k", &good).unwrap().valid);

        let bad_iss = Options { expected_iss: Some("other".into()), ..Default::default() };
        assert!(!verify(&t, "k", &bad_iss).unwrap().valid);

        let bad_aud = Options { expected_aud: Some("z".into()), ..Default::default() };
        assert!(!verify(&t, "k", &bad_aud).unwrap().valid);

        // string aud
        let t2 = hs256(r#"{"aud":"single"}"#, "k");
        let aud_ok = Options { expected_aud: Some("single".into()), ..Default::default() };
        assert!(verify(&t2, "k", &aud_ok).unwrap().valid);
    }

    #[test]
    fn expected_alg_mismatch_rejected() {
        let t = hs256(r#"{"sub":"x"}"#, "k");
        let o = Options { expected_alg: Some(Alg::Rs256), ..Default::default() };
        let r = verify(&t, "k", &o).unwrap();
        assert!(!r.valid);
        assert!(r.error.unwrap().contains("HS256"));
    }

    #[test]
    fn alg_none_rejected() {
        let header = B64URL.encode(br#"{"alg":"none"}"#);
        let payload = B64URL.encode(br#"{"sub":"x"}"#);
        let t = format!("{header}.{payload}.");
        assert!(verify(&t, "k", &opts_now(0)).is_err());
    }

    #[test]
    fn malformed_tokens_error() {
        assert!(verify("a.b", "k", &opts_now(0)).is_err()); // 2 parts
        assert!(verify("not-base64!.b.c", "k", &opts_now(0)).is_err());
        assert!(verify("aGVsbG8.b.c", "k", &opts_now(0)).is_err()); // header not json
    }

    #[test]
    fn decode_without_verify() {
        let t = hs256(r#"{"sub":"123"}"#, "k");
        let (h, p) = decode(&t).unwrap();
        assert_eq!(h["alg"], "HS256");
        assert_eq!(p["sub"], "123");
    }

    #[test]
    fn rs256_round_trip() {
        use rsa::pkcs1v15::SigningKey;
        use rsa::pkcs8::{EncodePublicKey, LineEnding};
        use rsa::signature::{SignatureEncoding, Signer};
        use rsa::{RsaPrivateKey, RsaPublicKey};
        use sha2::Sha256;

        let mut rng = rsa::rand_core::OsRng;
        let sk = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let pub_pem = RsaPublicKey::from(&sk).to_public_key_pem(LineEnding::LF).unwrap();

        let header = r#"{"alg":"RS256","typ":"JWT"}"#;
        let si = format!(
            "{}.{}",
            B64URL.encode(header.as_bytes()),
            B64URL.encode(br#"{"sub":"rsa"}"#)
        );
        let sig = SigningKey::<Sha256>::new(sk).sign(si.as_bytes()).to_vec();
        let token = format!("{si}.{}", B64URL.encode(sig));

        let r = verify(&token, &pub_pem, &opts_now(0)).unwrap();
        assert!(r.valid, "{:?}", r.error);
        assert_eq!(r.algorithm, "RS256");

        // tamper (sign with a DIFFERENT key) → well-formed but signature invalid
        let other = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let bad_sig = SigningKey::<Sha256>::new(other).sign(si.as_bytes()).to_vec();
        let bad = format!("{si}.{}", B64URL.encode(bad_sig));
        assert!(!verify(&bad, &pub_pem, &opts_now(0)).unwrap().valid);
    }

    #[test]
    fn es256_round_trip() {
        use p256::ecdsa::{signature::Signer, Signature, SigningKey};
        use p256::pkcs8::{EncodePublicKey, LineEnding};

        let sk = SigningKey::random(&mut p256::elliptic_curve::rand_core::OsRng);
        let vk = sk.verifying_key();
        let pub_pem = vk.to_public_key_pem(LineEnding::LF).unwrap();

        let header = r#"{"alg":"ES256","typ":"JWT"}"#;
        let si = format!(
            "{}.{}",
            B64URL.encode(header.as_bytes()),
            B64URL.encode(br#"{"sub":"ec"}"#)
        );
        let sig: Signature = sk.sign(si.as_bytes());
        let token = format!("{si}.{}", B64URL.encode(sig.to_bytes()));

        let r = verify(&token, &pub_pem, &opts_now(0)).unwrap();
        assert!(r.valid, "{:?}", r.error);
        assert_eq!(r.algorithm, "ES256");
    }

    #[test]
    fn parse_alg_forms() {
        assert_eq!(Alg::parse("hs256").unwrap(), Alg::Hs256);
        assert_eq!(Alg::parse("rs-512").unwrap(), Alg::Rs512);
        assert_eq!(Alg::parse("ES384").unwrap(), Alg::Es384);
        assert!(Alg::parse("none").is_err());
        assert!(Alg::parse("HS999").is_err());
    }
}
