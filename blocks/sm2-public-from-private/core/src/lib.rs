//! sm2-public-from-private core — derive the SM2 public key from a private key.
//!
//! Pure compute, shared by the chat skill block and the web page. No
//! wafer/wasm-bindgen deps. Given an SM2 private key (Chinese national standard
//! GM/T 0003, OSCCA curve sm2p256v1) — supplied either as a raw 32-byte scalar
//! in hex or as a PKCS#8 PEM — this computes the corresponding public key point
//! `Q = d·G` and serialises it.
//!
//! Deriving the public point is a single scalar multiplication of the curve's
//! base point, so it is fully deterministic (no RNG) and runs on every backend,
//! including the chat Service Worker and the browser page. The private key is
//! used only locally to compute the public point; it is never echoed back in
//! the output (only public key material is returned).
//!
//! Public key encodings produced:
//! - SEC1 uncompressed hex: `04 || x || y` (65 B → 130 hex chars);
//! - SEC1 compressed hex:   `02|03 || x`   (33 B → 66 hex chars);
//! - SPKI PEM (`-----BEGIN PUBLIC KEY-----`).

use serde::Serialize;
use sm2::elliptic_curve::sec1::ToEncodedPoint;
use sm2::pkcs8::{DecodePrivateKey, EncodePublicKey, LineEnding};

/// The derived SM2 public key in several encodings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublicKey {
    /// Public key, SubjectPublicKeyInfo (SPKI) PEM.
    pub public_pem: String,
    /// Public point, SEC1 uncompressed hex: `04 || x || y` (130 chars).
    pub public_point_hex: String,
    /// Public point, SEC1 compressed hex: `02|03 || x` (66 chars).
    pub public_point_hex_compressed: String,
    /// Affine x coordinate, hex (64 chars).
    pub x_hex: String,
    /// Affine y coordinate, hex (64 chars).
    pub y_hex: String,
    /// Curve identifier (always `sm2p256v1`).
    pub curve: String,
}

/// How to interpret the supplied private key string.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum InputFormat {
    /// Auto-detect: a `-----BEGIN`-style block is parsed as PKCS#8 PEM,
    /// otherwise the string is treated as a raw scalar in hex.
    Auto,
    /// Raw 32-byte private scalar in hexadecimal (a leading `0x` is allowed).
    Hex,
    /// PKCS#8 PEM (`-----BEGIN PRIVATE KEY-----`).
    Pem,
}

fn parse_input_format(s: &str) -> Result<InputFormat, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(InputFormat::Auto),
        "hex" | "scalar" | "raw" => Ok(InputFormat::Hex),
        "pem" | "pkcs8" => Ok(InputFormat::Pem),
        other => Err(format!(
            "invalid input_format '{other}': expected 'auto', 'hex', or 'pem'"
        )),
    }
}

/// Which representation of the derived public key to return.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum OutputFormat {
    /// A multi-line summary of every encoding (default).
    All,
    /// SEC1 uncompressed hex only.
    Uncompressed,
    /// SEC1 compressed hex only.
    Compressed,
    /// SPKI PEM only.
    Pem,
}

fn parse_output_format(s: &str) -> Result<OutputFormat, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "all" => Ok(OutputFormat::All),
        "uncompressed" | "hex" | "hex-uncompressed" => Ok(OutputFormat::Uncompressed),
        "compressed" | "hex-compressed" => Ok(OutputFormat::Compressed),
        "pem" | "spki" => Ok(OutputFormat::Pem),
        other => Err(format!(
            "invalid output_format '{other}': expected 'all', 'uncompressed', 'compressed', or 'pem'"
        )),
    }
}

/// Parse the private key string into an SM2 secret key per `input_format`.
fn parse_secret(private_key: &str, fmt: InputFormat) -> Result<sm2::SecretKey, String> {
    let trimmed = private_key.trim();
    if trimmed.is_empty() {
        return Err("private key is empty".to_string());
    }
    let looks_pem = trimmed.contains("-----BEGIN");
    let effective = match fmt {
        InputFormat::Auto => {
            if looks_pem {
                InputFormat::Pem
            } else {
                InputFormat::Hex
            }
        }
        other => other,
    };
    match effective {
        InputFormat::Pem => sm2::SecretKey::from_pkcs8_pem(trimmed)
            .map_err(|e| format!("invalid PKCS#8 PEM private key: {e}")),
        InputFormat::Hex => {
            // Tolerate a leading 0x and embedded whitespace.
            let cleaned: String = trimmed.split_whitespace().collect();
            let cleaned = cleaned
                .strip_prefix("0x")
                .or_else(|| cleaned.strip_prefix("0X"))
                .unwrap_or(&cleaned);
            let bytes = hex::decode(cleaned)
                .map_err(|e| format!("private key is not valid hex: {e}"))?;
            if bytes.len() != 32 {
                return Err(format!(
                    "private scalar must be 32 bytes (64 hex chars); got {} bytes",
                    bytes.len()
                ));
            }
            // from_slice rejects a scalar that is zero or >= the curve order.
            sm2::SecretKey::from_slice(&bytes).map_err(|_| {
                "private scalar is out of range for sm2p256v1 (must be in [1, n-1])".to_string()
            })
        }
        InputFormat::Auto => unreachable!(),
    }
}

/// Derive the public key from `private_key`, returning every encoding.
pub fn derive(private_key: &str, input_format: &str) -> Result<PublicKey, String> {
    let in_fmt = parse_input_format(input_format)?;
    let secret = parse_secret(private_key, in_fmt)?;
    let public = secret.public_key();

    let public_pem = public
        .to_public_key_pem(LineEnding::LF)
        .map_err(|e| format!("failed to encode public key: {e}"))?;
    let uncompressed = public.to_encoded_point(false);
    let compressed = public.to_encoded_point(true);
    // x/y are always present on a valid (non-identity) public point.
    let x_hex = uncompressed
        .x()
        .map(hex::encode)
        .unwrap_or_default();
    let y_hex = uncompressed
        .y()
        .map(hex::encode)
        .unwrap_or_default();

    Ok(PublicKey {
        public_pem,
        public_point_hex: hex::encode(uncompressed.as_bytes()),
        public_point_hex_compressed: hex::encode(compressed.as_bytes()),
        x_hex,
        y_hex,
        curve: "sm2p256v1".to_string(),
    })
}

/// Derive the public key and render it per `output_format` as a single string
/// (used by the CLI/page text surface). `all` returns a labelled multi-line
/// summary of every encoding.
pub fn derive_formatted(
    private_key: &str,
    input_format: &str,
    output_format: &str,
) -> Result<String, String> {
    let out_fmt = parse_output_format(output_format)?;
    let pk = derive(private_key, input_format)?;
    Ok(match out_fmt {
        OutputFormat::Uncompressed => pk.public_point_hex,
        OutputFormat::Compressed => pk.public_point_hex_compressed,
        OutputFormat::Pem => pk.public_pem.trim_end().to_string(),
        OutputFormat::All => format!(
            "{}\nPublic point (hex, uncompressed): {}\nPublic point (hex, compressed):   {}\nx: {}\ny: {}\nCurve: {}",
            pk.public_pem.trim_end(),
            pk.public_point_hex,
            pk.public_point_hex_compressed,
            pk.x_hex,
            pk.y_hex,
            pk.curve,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // GM/T 0003.5 SM2 standard worked example. Private scalar d and the
    // corresponding public point (x, y) on sm2p256v1.
    const D: &str = "3945208F7B2144B13F36E38AC6D39F95889393692860B51A42FB81EF4DF7C5B8";
    const X: &str = "09F9DF311E5421A150DD7D161E4BC5C672179FAD1833FC076BB08FF356F35020";
    const Y: &str = "CCEA490CE26775A52DC6EA718CC1AA600AED05FBF35E084A6632F6072DA9AD13";

    #[test]
    fn derives_standard_vector() {
        let pk = derive(D, "hex").unwrap();
        assert_eq!(pk.x_hex, X.to_lowercase());
        assert_eq!(pk.y_hex, Y.to_lowercase());
        let expected_uncompressed = format!("04{}{}", X.to_lowercase(), Y.to_lowercase());
        assert_eq!(pk.public_point_hex, expected_uncompressed);
        assert_eq!(pk.curve, "sm2p256v1");
    }

    #[test]
    fn compressed_point_shape() {
        let pk = derive(D, "auto").unwrap();
        assert_eq!(pk.public_point_hex_compressed.len(), 66);
        // y is odd here → 03 prefix; either way it must be 02 or 03.
        assert!(
            pk.public_point_hex_compressed.starts_with("02")
                || pk.public_point_hex_compressed.starts_with("03")
        );
        // Compressed x must equal the uncompressed x.
        assert_eq!(&pk.public_point_hex_compressed[2..], &pk.x_hex);
    }

    #[test]
    fn uncompressed_shape() {
        let pk = derive(D, "hex").unwrap();
        assert_eq!(pk.public_point_hex.len(), 130);
        assert!(pk.public_point_hex.starts_with("04"));
    }

    #[test]
    fn accepts_0x_prefix_and_whitespace() {
        let spaced = format!("0x{}", D);
        let a = derive(&spaced, "hex").unwrap();
        let b = derive(D, "hex").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn pem_round_trips_with_hex() {
        // Derive the public PEM from the hex scalar, then re-parse the secret
        // from a PKCS#8 PEM built for the same scalar and confirm both paths
        // agree.
        use sm2::pkcs8::EncodePrivateKey;
        let bytes = hex::decode(D).unwrap();
        let sk = sm2::SecretKey::from_slice(&bytes).unwrap();
        let priv_pem = sk.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();

        let from_hex = derive(D, "hex").unwrap();
        let from_pem = derive(&priv_pem, "pem").unwrap();
        assert_eq!(from_hex, from_pem);
        // Auto-detect should classify the PEM as PEM too.
        let from_auto = derive(&priv_pem, "auto").unwrap();
        assert_eq!(from_auto, from_pem);
    }

    #[test]
    fn public_pem_shape() {
        let pk = derive(D, "hex").unwrap();
        assert!(pk.public_pem.starts_with("-----BEGIN PUBLIC KEY-----"));
        assert!(pk
            .public_pem
            .trim_end()
            .ends_with("-----END PUBLIC KEY-----"));
    }

    #[test]
    fn formatted_selectors() {
        let pk = derive(D, "hex").unwrap();
        assert_eq!(
            derive_formatted(D, "hex", "uncompressed").unwrap(),
            pk.public_point_hex
        );
        assert_eq!(
            derive_formatted(D, "hex", "compressed").unwrap(),
            pk.public_point_hex_compressed
        );
        assert_eq!(
            derive_formatted(D, "hex", "pem").unwrap(),
            pk.public_pem.trim_end()
        );
        let all = derive_formatted(D, "hex", "all").unwrap();
        assert!(all.contains("-----BEGIN PUBLIC KEY-----"));
        assert!(all.contains(&pk.public_point_hex));
        assert!(all.contains(&pk.public_point_hex_compressed));
        assert!(all.contains("sm2p256v1"));
    }

    #[test]
    fn formatted_default_is_all() {
        assert_eq!(
            derive_formatted(D, "hex", "").unwrap(),
            derive_formatted(D, "hex", "all").unwrap()
        );
    }

    #[test]
    fn rejects_empty() {
        assert!(derive("", "auto").is_err());
        assert!(derive("   ", "hex").is_err());
    }

    #[test]
    fn rejects_bad_hex() {
        assert!(derive("zzzz", "hex").is_err());
    }

    #[test]
    fn rejects_wrong_length() {
        // 31 bytes.
        assert!(derive(&"ab".repeat(31), "hex").is_err());
        // 33 bytes.
        assert!(derive(&"ab".repeat(33), "hex").is_err());
    }

    #[test]
    fn rejects_zero_scalar() {
        assert!(derive(&"00".repeat(32), "hex").is_err());
    }

    #[test]
    fn rejects_bad_input_format() {
        assert!(derive(D, "der").is_err());
    }

    #[test]
    fn rejects_bad_output_format() {
        assert!(derive_formatted(D, "hex", "binary").is_err());
    }

    #[test]
    fn rejects_non_pem_as_pem() {
        assert!(derive(D, "pem").is_err());
    }
}
