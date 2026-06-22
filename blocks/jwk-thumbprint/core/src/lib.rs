//! gizza-ai/jwk-thumbprint core — compute the RFC 7638 SHA-256 thumbprint of a
//! JSON Web Key. The thumbprint is base64url(SHA-256(canonical-JSON)) over only
//! the required members for the key type, in lexicographic order, with no
//! whitespace. Pure-Rust (serde_json + sha2 + base64).

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Thumbprint {
    pub kty: String,
    /// The exact canonical JSON that was hashed (RFC 7638).
    pub canonical: String,
    /// base64url(SHA-256(canonical)) — the thumbprint / `kid`.
    pub thumbprint: String,
}

/// Required members per key type, already in lexicographic order.
fn required_members(kty: &str) -> Result<&'static [&'static str], String> {
    match kty {
        "RSA" => Ok(&["e", "kty", "n"]),
        "EC" => Ok(&["crv", "kty", "x", "y"]),
        "OKP" => Ok(&["crv", "kty", "x"]),
        "oct" => Ok(&["k", "kty"]),
        other => Err(format!("unsupported key type kty='{other}' (RSA, EC, OKP, or oct)")),
    }
}

/// Compute the RFC 7638 thumbprint of the JWK in `jwk_json`.
pub fn thumbprint(jwk_json: &str) -> Result<Thumbprint, String> {
    if jwk_json.trim().is_empty() {
        return Err("no JWK input".into());
    }
    let v: serde_json::Value =
        serde_json::from_str(jwk_json).map_err(|e| format!("invalid JSON: {e}"))?;
    let obj = v.as_object().ok_or("JWK must be a JSON object")?;
    let kty = obj
        .get("kty")
        .and_then(|x| x.as_str())
        .ok_or("JWK is missing the required string member 'kty'")?
        .to_string();

    let members = required_members(&kty)?;
    // Build the canonical JSON: {"k1":"v1",...} sorted, compact, escaped.
    let mut parts = Vec::with_capacity(members.len());
    for &k in members {
        let val = obj
            .get(k)
            .ok_or_else(|| format!("JWK of type {kty} is missing required member '{k}'"))?;
        let val_str = val
            .as_str()
            .ok_or_else(|| format!("JWK member '{k}' must be a string"))?;
        // serde_json::to_string escapes both the key and the value correctly.
        let ks = serde_json::to_string(k).map_err(|e| format!("encode: {e}"))?;
        let vs = serde_json::to_string(val_str).map_err(|e| format!("encode: {e}"))?;
        parts.push(format!("{ks}:{vs}"));
    }
    let canonical = format!("{{{}}}", parts.join(","));

    let digest = Sha256::digest(canonical.as_bytes());
    let thumbprint = B64URL.encode(digest);

    Ok(Thumbprint { kty, canonical, thumbprint })
}

#[cfg(test)]
mod tests {
    use super::*;

    // The canonical RFC 7638 §3.1 example.
    const RFC_RSA: &str = r#"{"kty":"RSA","n":"0vx7agoebGcQSuuPiLJXZptN9nndrQmbXEps2aiAFbWhM78LhWx4cbbfAAtVT86zwu1RK7aPFFxuhDR1L6tSoc_BJECPebWKRXjBZCiFV4n3oknjhMstn64tZ_2W-5JsGY4Hc5n9yBXArwl93lqt7_RN5w6Cf0h4QyQ5v-65YGjQR0_FDW2QvzqY368QQMicAtaSqzs8KJZgnYb9c7d0zgdAZHzu6qMQvRL5hajrn1n91CbOpbISD08qNLyrdkt-bFTWhAI4vMQFh6WeZu0fM4lFd2NcRwr3XPksINHaQ-G_xBniIqbw0Ls1jF44-csFCur-kEgU8awapJzKnqDKgw","e":"AQAB","alg":"RS256","kid":"2011-04-29"}"#;

    #[test]
    fn rfc7638_rsa_example() {
        let t = thumbprint(RFC_RSA).unwrap();
        assert_eq!(t.thumbprint, "NzbLsXh8uDCcd-6MNwXF4W_7noWXFZAfHkxZsRGC9Xs");
        assert_eq!(t.kty, "RSA");
        // canonical drops alg/kid, keeps e,kty,n in order.
        assert!(t.canonical.starts_with(r#"{"e":"AQAB","kty":"RSA","n":""#));
    }

    #[test]
    fn ec_key() {
        let jwk = r#"{"kty":"EC","crv":"P-256","x":"abc","y":"def","use":"sig"}"#;
        let t = thumbprint(jwk).unwrap();
        assert_eq!(t.canonical, r#"{"crv":"P-256","kty":"EC","x":"abc","y":"def"}"#);
        assert_eq!(t.thumbprint.len(), 43); // 32 bytes base64url-nopad
    }

    #[test]
    fn okp_key() {
        let jwk = r#"{"kty":"OKP","crv":"Ed25519","x":"11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo"}"#;
        let t = thumbprint(jwk).unwrap();
        assert_eq!(t.canonical, r#"{"crv":"Ed25519","kty":"OKP","x":"11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo"}"#);
    }

    #[test]
    fn oct_key() {
        let t = thumbprint(r#"{"kty":"oct","k":"c2VjcmV0"}"#).unwrap();
        assert_eq!(t.canonical, r#"{"k":"c2VjcmV0","kty":"oct"}"#);
    }

    #[test]
    fn errors() {
        assert!(thumbprint("").is_err());
        assert!(thumbprint("{bad}").is_err());
        assert!(thumbprint(r#"{"foo":1}"#).is_err()); // no kty
        assert!(thumbprint(r#"{"kty":"RSA","e":"AQAB"}"#).is_err()); // missing n
        assert!(thumbprint(r#"{"kty":"XYZ"}"#).is_err()); // unsupported
    }
}
