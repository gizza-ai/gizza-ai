//! gizza-ai/jwt-sign core — build and sign a JSON Web Token (JWT/JWS compact
//! serialization) from a header + payload using an HMAC secret (HS256/384/512),
//! an RSA private key (RS256/384/512, RSASSA-PKCS1-v1_5) or an EC private key
//! (ES256/384, ECDSA). Pure-Rust; no wafer / wasm-bindgen deps.
//!
//! Output is the compact JWT string `base64url(header).base64url(payload).base64url(signature)`.

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine;
use serde_json::Value;

/// Supported JWS signing algorithms.
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
            "HS256" | "" => Ok(Alg::Hs256),
            "HS384" => Ok(Alg::Hs384),
            "HS512" => Ok(Alg::Hs512),
            "RS256" => Ok(Alg::Rs256),
            "RS384" => Ok(Alg::Rs384),
            "RS512" => Ok(Alg::Rs512),
            "ES256" => Ok(Alg::Es256),
            "ES384" => Ok(Alg::Es384),
            other => Err(format!(
                "unknown algorithm '{other}' (use HS256/384/512, RS256/384/512, or ES256/384)"
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

/// Parse a JSON object string, returning a serde_json Map (must be an object).
fn parse_object(label: &str, s: &str) -> Result<serde_json::Map<String, Value>, String> {
    let v: Value = serde_json::from_str(s.trim())
        .map_err(|e| format!("{label} is not valid JSON: {e}"))?;
    match v {
        Value::Object(m) => Ok(m),
        _ => Err(format!("{label} must be a JSON object (e.g. {{\"sub\":\"123\"}})")),
    }
}

/// Build the JWT header object: merges the user's header (if any) with the
/// required `alg`/`typ` claims. `alg` always reflects the chosen algorithm;
/// `typ` defaults to "JWT" unless the caller already set it.
fn build_header(alg: Alg, user_header: &str) -> Result<String, String> {
    let mut map = if user_header.trim().is_empty() {
        serde_json::Map::new()
    } else {
        parse_object("header", user_header)?
    };
    map.insert("alg".to_string(), Value::String(alg.name().to_string()));
    map.entry("typ".to_string())
        .or_insert_with(|| Value::String("JWT".to_string()));
    serde_json::to_string(&Value::Object(map)).map_err(|e| e.to_string())
}

fn hmac_sign(key: &[u8], data: &[u8], alg: Alg) -> Vec<u8> {
    use hmac::{Hmac, Mac};
    use sha2::{Sha256, Sha384, Sha512};
    match alg {
        Alg::Hs256 => {
            let mut m = <Hmac<Sha256>>::new_from_slice(key).expect("hmac key");
            m.update(data);
            m.finalize().into_bytes().to_vec()
        }
        Alg::Hs384 => {
            let mut m = <Hmac<Sha384>>::new_from_slice(key).expect("hmac key");
            m.update(data);
            m.finalize().into_bytes().to_vec()
        }
        Alg::Hs512 => {
            let mut m = <Hmac<Sha512>>::new_from_slice(key).expect("hmac key");
            m.update(data);
            m.finalize().into_bytes().to_vec()
        }
        _ => unreachable!(),
    }
}

fn rsa_sign(pem: &str, data: &[u8], alg: Alg) -> Result<Vec<u8>, String> {
    use rsa::pkcs1::DecodeRsaPrivateKey;
    use rsa::pkcs8::DecodePrivateKey;
    use rsa::signature::{SignatureEncoding, Signer};
    use rsa::RsaPrivateKey;
    use sha2::{Sha256, Sha384, Sha512};

    if !pem.contains("PRIVATE KEY") {
        return Err("no RSA private key found — paste a PEM '-----BEGIN PRIVATE KEY-----' (PKCS#8) or '-----BEGIN RSA PRIVATE KEY-----' (PKCS#1) block".into());
    }
    let key = RsaPrivateKey::from_pkcs8_pem(pem)
        .or_else(|_| RsaPrivateKey::from_pkcs1_pem(pem))
        .map_err(|e| format!("invalid RSA private key: {e}"))?;
    let sig = match alg {
        Alg::Rs256 => rsa::pkcs1v15::SigningKey::<Sha256>::new(key).sign(data).to_vec(),
        Alg::Rs384 => rsa::pkcs1v15::SigningKey::<Sha384>::new(key).sign(data).to_vec(),
        Alg::Rs512 => rsa::pkcs1v15::SigningKey::<Sha512>::new(key).sign(data).to_vec(),
        _ => unreachable!(),
    };
    Ok(sig)
}

fn ecdsa_sign(pem: &str, data: &[u8], alg: Alg) -> Result<Vec<u8>, String> {
    if !pem.contains("PRIVATE KEY") {
        return Err("no EC private key found — paste a PEM '-----BEGIN PRIVATE KEY-----' (PKCS#8) block".into());
    }
    // JWS ES* signatures are fixed-length raw r||s (IEEE P1363), NOT DER.
    match alg {
        Alg::Es256 => {
            use p256::ecdsa::{signature::Signer, Signature, SigningKey};
            use p256::pkcs8::DecodePrivateKey;
            let sk = SigningKey::from_pkcs8_pem(pem)
                .map_err(|e| format!("invalid P-256 private key (ES256 needs a P-256/prime256v1 key): {e}"))?;
            let sig: Signature = sk.sign(data);
            Ok(sig.to_bytes().to_vec()) // 64 bytes r||s
        }
        Alg::Es384 => {
            use p384::ecdsa::{signature::Signer, Signature, SigningKey};
            use p384::pkcs8::DecodePrivateKey;
            let sk = SigningKey::from_pkcs8_pem(pem)
                .map_err(|e| format!("invalid P-384 private key (ES384 needs a P-384/secp384r1 key): {e}"))?;
            let sig: Signature = sk.sign(data);
            Ok(sig.to_bytes().to_vec()) // 96 bytes r||s
        }
        _ => unreachable!(),
    }
}

/// Build and sign a JWT.
///
/// - `header_json`: optional JSON object for extra header fields (may be empty;
///   `alg` is always set/overwritten, `typ` defaults to "JWT").
/// - `payload_json`: the JSON object claims set (required, must be an object).
/// - `secret`: HMAC secret (HS*) or PEM private key (RS*/ES*).
/// - `alg`: the signing algorithm.
///
/// Returns the compact JWT string.
pub fn sign(
    header_json: &str,
    payload_json: &str,
    secret: &str,
    alg: Alg,
) -> Result<String, String> {
    // Payload must be a JSON object.
    let payload_map = parse_object("payload", payload_json)?;
    let payload_compact =
        serde_json::to_string(&Value::Object(payload_map)).map_err(|e| e.to_string())?;
    let header_compact = build_header(alg, header_json)?;

    let signing_input = format!(
        "{}.{}",
        B64URL.encode(header_compact.as_bytes()),
        B64URL.encode(payload_compact.as_bytes())
    );

    let sig: Vec<u8> = if alg.is_hmac() {
        if secret.is_empty() {
            return Err("HMAC secret is empty — provide the shared secret".into());
        }
        hmac_sign(secret.as_bytes(), signing_input.as_bytes(), alg)
    } else if matches!(alg, Alg::Rs256 | Alg::Rs384 | Alg::Rs512) {
        rsa_sign(secret, signing_input.as_bytes(), alg)?
    } else {
        ecdsa_sign(secret, signing_input.as_bytes(), alg)?
    };

    Ok(format!("{signing_input}.{}", B64URL.encode(sig)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hs256_structure_vector() {
        let header = r#"{"typ":"JWT","alg":"HS256"}"#;
        let payload =
            r#"{"iss":"joe","exp":1300819380,"http://example.com/is_root":true}"#;
        let jwt = sign(header, payload, "secret", Alg::Hs256).unwrap();
        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3);
        let h: Value =
            serde_json::from_slice(&B64URL.decode(parts[0]).unwrap()).unwrap();
        assert_eq!(h["alg"], "HS256");
        assert_eq!(h["typ"], "JWT");
        let p: Value =
            serde_json::from_slice(&B64URL.decode(parts[1]).unwrap()).unwrap();
        assert_eq!(p["iss"], "joe");
        assert_eq!(p["exp"], 1300819380i64);
    }

    #[test]
    fn hs256_verifies_with_hmac() {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        let jwt = sign("", r#"{"sub":"1234567890","name":"John Doe"}"#, "my-secret", Alg::Hs256)
            .unwrap();
        let (signing_input, sig_b64) = jwt.rsplit_once('.').unwrap();
        let mut m = <Hmac<Sha256>>::new_from_slice(b"my-secret").unwrap();
        m.update(signing_input.as_bytes());
        let expected = m.finalize().into_bytes().to_vec();
        let got = B64URL.decode(sig_b64).unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn header_alg_is_overwritten_and_typ_defaults() {
        let jwt = sign(r#"{"alg":"none","kid":"k1"}"#, r#"{"a":1}"#, "s", Alg::Hs512).unwrap();
        let header_b64 = jwt.split('.').next().unwrap();
        let h: Value = serde_json::from_slice(&B64URL.decode(header_b64).unwrap()).unwrap();
        assert_eq!(h["alg"], "HS512");
        assert_eq!(h["typ"], "JWT");
        assert_eq!(h["kid"], "k1");
    }

    #[test]
    fn deterministic_hmac() {
        let a = sign("", r#"{"x":1}"#, "k", Alg::Hs256).unwrap();
        let b = sign("", r#"{"x":1}"#, "k", Alg::Hs256).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn rs256_verifies() {
        use rsa::pkcs1v15::{Signature, VerifyingKey};
        use rsa::pkcs8::{DecodePublicKey, EncodePublicKey, LineEnding};
        use rsa::signature::Verifier;
        use rsa::{RsaPrivateKey, RsaPublicKey};
        use sha2::Sha256;
        use rsa::pkcs8::EncodePrivateKey;

        let mut rng = rsa::rand_core::OsRng;
        let sk = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let pem = sk.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
        let pub_pem = RsaPublicKey::from(&sk).to_public_key_pem(LineEnding::LF).unwrap();

        let jwt = sign("", r#"{"sub":"abc"}"#, &pem, Alg::Rs256).unwrap();
        let (signing_input, sig_b64) = jwt.rsplit_once('.').unwrap();
        let pubk = RsaPublicKey::from_public_key_pem(&pub_pem).unwrap();
        let vk = VerifyingKey::<Sha256>::new(pubk);
        let sig = Signature::try_from(B64URL.decode(sig_b64).unwrap().as_slice()).unwrap();
        vk.verify(signing_input.as_bytes(), &sig).unwrap();
    }

    #[test]
    fn es256_verifies_raw_rs() {
        use p256::ecdsa::{signature::Verifier, Signature, SigningKey, VerifyingKey};
        use p256::pkcs8::{DecodePrivateKey, EncodePrivateKey, LineEnding};

        let sk = SigningKey::random(&mut p256::elliptic_curve::rand_core::OsRng);
        let pem = sk.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
        let jwt = sign("", r#"{"sub":"ec"}"#, &pem, Alg::Es256).unwrap();
        let (signing_input, sig_b64) = jwt.rsplit_once('.').unwrap();
        let sig_bytes = B64URL.decode(sig_b64).unwrap();
        assert_eq!(sig_bytes.len(), 64, "ES256 signature is raw r||s = 64 bytes");
        let vk = VerifyingKey::from(&SigningKey::from_pkcs8_pem(&pem).unwrap());
        let sig = Signature::from_slice(&sig_bytes).unwrap();
        vk.verify(signing_input.as_bytes(), &sig).unwrap();
    }

    #[test]
    fn errors() {
        assert!(sign("", "not json", "k", Alg::Hs256).is_err());
        assert!(sign("", "[1,2,3]", "k", Alg::Hs256).is_err()); // payload not object
        assert!(sign("not json", r#"{"a":1}"#, "k", Alg::Hs256).is_err()); // header not object
        assert!(sign("", r#"{"a":1}"#, "", Alg::Hs256).is_err()); // empty secret
        assert!(sign("", r#"{"a":1}"#, "not a key", Alg::Rs256).is_err()); // bad rsa key
        assert!(Alg::parse("HS999").is_err());
        assert_eq!(Alg::parse("es256").unwrap(), Alg::Es256);
        assert_eq!(Alg::parse("rs-512").unwrap(), Alg::Rs512);
    }
}
