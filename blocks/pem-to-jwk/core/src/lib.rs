//! gizza-ai/pem-to-jwk core — convert a PEM-encoded key into a JSON Web Key
//! (JWK, RFC 7517 / 7518). Pure-Rust (RustCrypto): no wafer/wasm-bindgen deps.
//!
//! Supports RSA (PKCS#1 / PKCS#8 / SPKI) and EC over the NIST curves P-256,
//! P-384 and P-521 (SEC1 / PKCS#8 / SPKI). Public keys yield a public JWK;
//! private keys yield a private JWK (with the private components) — the standard
//! representation a public key can be derived from.

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine;
use serde_json::{json, Value};

use rsa::pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey};
use rsa::pkcs8::{DecodePrivateKey, DecodePublicKey};
use rsa::traits::{PrivateKeyParts, PublicKeyParts};
use rsa::{RsaPrivateKey, RsaPublicKey};

use p256::elliptic_curve::sec1::ToEncodedPoint;

/// Base64url-encode (no padding), as JWK requires for all binary members.
fn b64u(bytes: &[u8]) -> String {
    B64URL.encode(bytes)
}

/// Big-endian unsigned bytes of an RSA `BigUint`, base64url-encoded.
fn b64u_uint(n: &rsa::BigUint) -> String {
    b64u(&n.to_bytes_be())
}

/// Convert a PEM string into a JWK. The result is a JSON object; `kty` is `"RSA"`
/// or `"EC"`. Returns a human-readable error if the PEM can't be parsed as any
/// supported key.
pub fn run(input: &str) -> Result<Value, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("no PEM input — paste a PEM-encoded key (-----BEGIN ...-----)".into());
    }
    let block = pem::parse(input.as_bytes())
        .map_err(|e| format!("not valid PEM: {e}"))?;
    let label = block.tag().to_string();
    let der = block.contents();

    // Dispatch on the PEM label.
    match label.as_str() {
        "RSA PRIVATE KEY" => rsa_private_from_pkcs1(der),
        "RSA PUBLIC KEY" => rsa_public_from_pkcs1(der),
        "EC PRIVATE KEY" => ec_private_from_sec1(der),
        "PRIVATE KEY" => private_from_pkcs8(der),
        "PUBLIC KEY" => public_from_spki(der),
        other => Err(format!(
            "unsupported PEM label '{other}'. Supported: RSA/EC PRIVATE KEY, PRIVATE KEY (PKCS#8), RSA PUBLIC KEY, PUBLIC KEY (SPKI)"
        )),
    }
}

// ---- RSA ----------------------------------------------------------------

fn rsa_public_jwk(key: &RsaPublicKey) -> Value {
    json!({
        "kty": "RSA",
        "n": b64u_uint(key.n()),
        "e": b64u_uint(key.e()),
    })
}

fn rsa_private_jwk(key: &RsaPrivateKey) -> Result<Value, String> {
    let primes = key.primes();
    if primes.len() != 2 {
        return Err("multi-prime RSA keys are not representable as a standard JWK".into());
    }
    let (p, q) = (&primes[0], &primes[1]);
    // CRT params. rsa exposes dp/dq/qinv as Options; compute deterministically
    // from the primes if absent.
    let one = rsa::BigUint::from(1u8);
    let dp = key.dp().cloned().unwrap_or_else(|| key.d() % (p - &one));
    let dq = key.dq().cloned().unwrap_or_else(|| key.d() % (q - &one));
    let qinv = key
        .qinv()
        .and_then(|v| v.to_biguint())
        .ok_or("RSA key is missing the CRT coefficient (qi)")?;
    Ok(json!({
        "kty": "RSA",
        "n": b64u_uint(key.n()),
        "e": b64u_uint(key.e()),
        "d": b64u_uint(key.d()),
        "p": b64u_uint(p),
        "q": b64u_uint(q),
        "dp": b64u_uint(&dp),
        "dq": b64u_uint(&dq),
        "qi": b64u_uint(&qinv),
    }))
}

fn rsa_public_from_pkcs1(der: &[u8]) -> Result<Value, String> {
    let key = RsaPublicKey::from_pkcs1_der(der)
        .map_err(|e| format!("invalid PKCS#1 RSA public key: {e}"))?;
    Ok(rsa_public_jwk(&key))
}

fn rsa_private_from_pkcs1(der: &[u8]) -> Result<Value, String> {
    let key = RsaPrivateKey::from_pkcs1_der(der)
        .map_err(|e| format!("invalid PKCS#1 RSA private key: {e}"))?;
    rsa_private_jwk(&key)
}

// ---- generic PKCS#8 / SPKI (RSA or EC) ----------------------------------

/// Curve OIDs we recognise in an SPKI/PKCS#8 EC key.
const OID_EC_PUBLIC_KEY: &str = "1.2.840.10045.2.1";
const OID_P256: &str = "1.2.840.10045.3.1.7";
const OID_P384: &str = "1.3.132.0.34";
const OID_P521: &str = "1.3.132.0.35";

fn private_from_pkcs8(der: &[u8]) -> Result<Value, String> {
    // Inspect the algorithm OID to route RSA vs EC-by-curve.
    use pkcs8::der::Decode;
    use pkcs8::PrivateKeyInfo;
    let pki = PrivateKeyInfo::from_der(der)
        .map_err(|e| format!("invalid PKCS#8 key: {e}"))?;
    let alg = pki.algorithm.oid.to_string();
    if alg == OID_EC_PUBLIC_KEY {
        let crv = pki
            .algorithm
            .parameters_oid()
            .map_err(|_| "EC PKCS#8 key has no named-curve parameter".to_string())?
            .to_string();
        return ec_private_from_pkcs8_der(der, &crv);
    }
    // Otherwise treat as RSA.
    let key = RsaPrivateKey::from_pkcs8_der(der)
        .map_err(|e| format!("unsupported PKCS#8 key (not RSA or recognised EC): {e}"))?;
    rsa_private_jwk(&key)
}

fn public_from_spki(der: &[u8]) -> Result<Value, String> {
    use spki::der::Decode;
    use spki::SubjectPublicKeyInfoRef;
    let spki = SubjectPublicKeyInfoRef::from_der(der)
        .map_err(|e| format!("invalid SPKI public key: {e}"))?;
    let alg = spki.algorithm.oid.to_string();
    if alg == OID_EC_PUBLIC_KEY {
        let crv = spki
            .algorithm
            .parameters_oid()
            .map_err(|_| "EC public key has no named-curve parameter".to_string())?
            .to_string();
        return ec_public_from_spki_der(der, &crv);
    }
    let key = RsaPublicKey::from_public_key_der(der)
        .map_err(|e| format!("unsupported SPKI key (not RSA or recognised EC): {e}"))?;
    Ok(rsa_public_jwk(&key))
}

// ---- EC -----------------------------------------------------------------

/// Build an EC public JWK from uncompressed SEC1 point coordinates.
fn ec_public_jwk(crv: &str, x: &[u8], y: &[u8]) -> Value {
    json!({ "kty": "EC", "crv": crv, "x": b64u(x), "y": b64u(y) })
}

/// `EC PRIVATE KEY` (SEC1) — try each supported curve.
fn ec_private_from_sec1(der: &[u8]) -> Result<Value, String> {
    if let Ok(sk) = p256::SecretKey::from_sec1_der(der) {
        return Ok(ec_private_jwk_p256(&sk));
    }
    if let Ok(sk) = p384::SecretKey::from_sec1_der(der) {
        return Ok(ec_private_jwk_p384(&sk));
    }
    if let Ok(sk) = p521::SecretKey::from_sec1_der(der) {
        return Ok(ec_private_jwk_p521(&sk));
    }
    Err("EC private key is not on a supported curve (P-256, P-384, P-521)".into())
}

fn ec_private_from_pkcs8_der(der: &[u8], crv_oid: &str) -> Result<Value, String> {
    match crv_oid {
        OID_P256 => p256::SecretKey::from_pkcs8_der(der)
            .map(|sk| ec_private_jwk_p256(&sk))
            .map_err(|e| format!("invalid P-256 PKCS#8 key: {e}")),
        OID_P384 => p384::SecretKey::from_pkcs8_der(der)
            .map(|sk| ec_private_jwk_p384(&sk))
            .map_err(|e| format!("invalid P-384 PKCS#8 key: {e}")),
        OID_P521 => p521::SecretKey::from_pkcs8_der(der)
            .map(|sk| ec_private_jwk_p521(&sk))
            .map_err(|e| format!("invalid P-521 PKCS#8 key: {e}")),
        other => Err(format!("unsupported EC curve OID {other}")),
    }
}

fn ec_public_from_spki_der(der: &[u8], crv_oid: &str) -> Result<Value, String> {
    match crv_oid {
        OID_P256 => p256::PublicKey::from_public_key_der(der)
            .map(|pk| ec_public_jwk_p256(&pk))
            .map_err(|e| format!("invalid P-256 public key: {e}")),
        OID_P384 => p384::PublicKey::from_public_key_der(der)
            .map(|pk| ec_public_jwk_p384(&pk))
            .map_err(|e| format!("invalid P-384 public key: {e}")),
        OID_P521 => p521::PublicKey::from_public_key_der(der)
            .map(|pk| ec_public_jwk_p521(&pk))
            .map_err(|e| format!("invalid P-521 public key: {e}")),
        other => Err(format!("unsupported EC curve OID {other}")),
    }
}

// Per-curve coordinate extraction. Each curve crate has its own types, so these
// are near-identical but monomorphic.
macro_rules! ec_jwk_impls {
    ($crv:literal, $modpath:ident, $pubfn:ident, $privfn:ident) => {
        fn $pubfn(pk: &$modpath::PublicKey) -> Value {
            let pt = pk.to_encoded_point(false);
            ec_public_jwk(
                $crv,
                pt.x().map(|b| &b[..]).unwrap_or(&[]),
                pt.y().map(|b| &b[..]).unwrap_or(&[]),
            )
        }
        fn $privfn(sk: &$modpath::SecretKey) -> Value {
            let pt = sk.public_key().to_encoded_point(false);
            let mut v = ec_public_jwk(
                $crv,
                pt.x().map(|b| &b[..]).unwrap_or(&[]),
                pt.y().map(|b| &b[..]).unwrap_or(&[]),
            );
            v.as_object_mut()
                .unwrap()
                .insert("d".to_string(), Value::String(b64u(&sk.to_bytes())));
            v
        }
    };
}
ec_jwk_impls!("P-256", p256, ec_public_jwk_p256, ec_private_jwk_p256);
ec_jwk_impls!("P-384", p384, ec_public_jwk_p384, ec_private_jwk_p384);
ec_jwk_impls!("P-521", p521, ec_public_jwk_p521, ec_private_jwk_p521);

#[cfg(test)]
mod tests {
    use super::*;

    const RSA_PRIV: &str = include_str!("../tests/rsa_priv.pem");
    const RSA_PUB: &str = include_str!("../tests/rsa_pub.pem");
    const EC_P256_PRIV: &str = include_str!("../tests/ec256_priv.pem");
    const EC_P256_PUB: &str = include_str!("../tests/ec256_pub.pem");
    const EC_P384_PRIV: &str = include_str!("../tests/ec384_priv.pem");

    #[test]
    fn rsa_public_jwk_shape() {
        let v = run(RSA_PUB).unwrap();
        assert_eq!(v["kty"], "RSA");
        assert!(v["n"].as_str().unwrap().len() > 300);
        assert_eq!(v["e"], "AQAB"); // 65537
        assert!(v.get("d").is_none());
    }

    #[test]
    fn rsa_private_jwk_has_crt() {
        let v = run(RSA_PRIV).unwrap();
        assert_eq!(v["kty"], "RSA");
        for k in ["n", "e", "d", "p", "q", "dp", "dq", "qi"] {
            assert!(v[k].as_str().is_some(), "missing {k}");
        }
        let pubv = run(RSA_PUB).unwrap();
        assert_eq!(v["n"], pubv["n"]);
    }

    #[test]
    fn ec_p256_public_and_private() {
        let pubv = run(EC_P256_PUB).unwrap();
        assert_eq!(pubv["kty"], "EC");
        assert_eq!(pubv["crv"], "P-256");
        assert_eq!(B64URL.decode(pubv["x"].as_str().unwrap()).unwrap().len(), 32);
        assert!(pubv.get("d").is_none());

        let privv = run(EC_P256_PRIV).unwrap();
        assert_eq!(privv["crv"], "P-256");
        assert_eq!(B64URL.decode(privv["d"].as_str().unwrap()).unwrap().len(), 32);
        assert_eq!(privv["x"], pubv["x"]);
        assert_eq!(privv["y"], pubv["y"]);
    }

    #[test]
    fn ec_p384_private() {
        let v = run(EC_P384_PRIV).unwrap();
        assert_eq!(v["crv"], "P-384");
        assert_eq!(B64URL.decode(v["d"].as_str().unwrap()).unwrap().len(), 48);
    }

    #[test]
    fn errors_clearly() {
        assert!(run("").is_err());
        assert!(run("not a pem").is_err());
        assert!(run("-----BEGIN WAT-----\nAAAA\n-----END WAT-----").is_err());
    }
}
