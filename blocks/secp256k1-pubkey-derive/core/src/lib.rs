//! secp256k1-pubkey-derive core — derive the secp256k1 **public key** point from a
//! private key. Shared by the chat skill block, the CLI, and the web page.
//!
//! Given a private key (hex or WIF), it performs the elliptic-curve scalar
//! multiplication `pubkey = privkey · G` on the secp256k1 curve and reports the
//! point in every common encoding:
//!
//! * **compressed** SEC1 point — 33 bytes, `02`/`03` ‖ X (prefix encodes Y parity),
//! * **uncompressed** SEC1 point — 65 bytes, `04` ‖ X ‖ Y,
//! * the raw **X** coordinate (32 bytes — this is also the Taproot/x-only key),
//! * the raw **Y** coordinate (32 bytes),
//! * the **Y parity** (even → `02`, odd → `03`).
//!
//! The public-key point is identical across chains (Bitcoin, Ethereum, Tron, …);
//! only the downstream address encoding differs, which is out of scope here.
//! Pure-Rust k256 (secp256k1) + bs58 (WIF), so it runs on every backend (native,
//! wasm32-wasip1, browser wasm) with no host calls and no RNG.

use k256::elliptic_curve::sec1::ToEncodedPoint;

/// Which representation to return. `All` prints every field; the others return a
/// single bare hex value (convenient for copy-paste / scripting).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    All,
    Compressed,
    Uncompressed,
    X,
    Y,
}

impl Format {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "all" => Ok(Self::All),
            "compressed" | "comp" => Ok(Self::Compressed),
            "uncompressed" | "uncomp" => Ok(Self::Uncompressed),
            "x" | "x_only" | "xonly" => Ok(Self::X),
            "y" => Ok(Self::Y),
            other => Err(format!(
                "unknown format '{other}' (use all, compressed, uncompressed, x, or y)"
            )),
        }
    }
}

/// Parse a private key given as **hex** (64 chars, `0x` prefix / spaces /
/// underscores tolerated) or **WIF** (base58check, `5…`/`K…`/`L…` mainnet,
/// `9…`/`c…` testnet). Returns the raw 32-byte secret scalar.
///
/// The WIF's embedded network + compression flag are irrelevant to the public
/// key point (both compressed and uncompressed forms are always derivable), so
/// they're parsed for validation but otherwise ignored.
fn parse_secret(key: &str) -> Result<[u8; 32], String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("private key is required (64-char hex or WIF)".into());
    }
    let cleaned: String = trimmed
        .trim_start_matches("0x")
        .trim_start_matches("0X")
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && *c != '_')
        .collect();

    let looks_hex = !cleaned.is_empty() && cleaned.chars().all(|c| c.is_ascii_hexdigit());
    if looks_hex {
        if cleaned.len() != 64 {
            return Err(format!(
                "hex private key must be 32 bytes (64 hex chars), got {} chars",
                cleaned.len()
            ));
        }
        let bytes = hex::decode(&cleaned).map_err(|e| format!("invalid hex private key: {e}"))?;
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&bytes);
        return Ok(secret);
    }

    // Otherwise treat it as WIF (base58check).
    let payload = bs58::decode(trimmed)
        .with_check(None)
        .into_vec()
        .map_err(|_| {
            "key is neither valid 64-char hex nor a valid WIF (base58check failed)".to_string()
        })?;
    let secret_bytes = match payload.len() {
        33 => &payload[1..33],                        // version ‖ 32-byte key
        34 if payload[33] == 0x01 => &payload[1..33], // ‖ 0x01 compressed flag
        34 => {
            return Err(format!(
                "invalid WIF compression flag 0x{:02x} (expected 0x01)",
                payload[33]
            ))
        }
        n => {
            return Err(format!(
                "WIF payload has unexpected length {n} (expected 33 or 34 bytes)"
            ))
        }
    };
    let mut secret = [0u8; 32];
    secret.copy_from_slice(secret_bytes);
    Ok(secret)
}

/// Derive the public key of `key` and render it in `format`.
pub fn derive(key: &str, format: &str) -> Result<String, String> {
    let fmt = Format::parse(format)?;
    let secret = parse_secret(key)?;

    let sk = k256::SecretKey::from_slice(&secret).map_err(|_| {
        "private key is not a valid secp256k1 scalar (must be non-zero and below the curve order)"
            .to_string()
    })?;
    let pk = sk.public_key();
    let compressed = pk.to_encoded_point(true); // 33 bytes: 02/03 ‖ X
    let uncompressed = pk.to_encoded_point(false); // 65 bytes: 04 ‖ X ‖ Y

    let comp_hex = hex::encode(compressed.as_bytes());
    let uncomp_hex = hex::encode(uncompressed.as_bytes());
    let x_hex = hex::encode(&uncompressed.as_bytes()[1..33]);
    let y_hex = hex::encode(&uncompressed.as_bytes()[33..65]);
    // Prefix 0x02 = even Y, 0x03 = odd Y.
    let parity = if compressed.as_bytes()[0] == 0x02 {
        "even"
    } else {
        "odd"
    };

    Ok(match fmt {
        Format::Compressed => comp_hex,
        Format::Uncompressed => uncomp_hex,
        Format::X => x_hex,
        Format::Y => y_hex,
        Format::All => format!(
            "compressed: {comp_hex}\nuncompressed: {uncomp_hex}\nx: {x_hex}\ny: {y_hex}\ny_parity: {parity}"
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Private key = 1 → the secp256k1 generator point G (a canonical, widely
    // published test vector). G.y ends in 0x8 (even) → compressed prefix 02.
    const PRIV_ONE: &str = "0000000000000000000000000000000000000000000000000000000000000001";
    const G_X: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
    const G_Y: &str = "483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8";

    #[test]
    fn generator_point_all_fields() {
        let out = derive(PRIV_ONE, "all").unwrap();
        assert_eq!(
            out,
            format!(
                "compressed: 02{G_X}\nuncompressed: 04{G_X}{G_Y}\nx: {G_X}\ny: {G_Y}\ny_parity: even"
            )
        );
    }

    #[test]
    fn single_format_outputs_bare_hex() {
        assert_eq!(derive(PRIV_ONE, "compressed").unwrap(), format!("02{G_X}"));
        assert_eq!(
            derive(PRIV_ONE, "uncompressed").unwrap(),
            format!("04{G_X}{G_Y}")
        );
        assert_eq!(derive(PRIV_ONE, "x").unwrap(), G_X);
        assert_eq!(derive(PRIV_ONE, "y").unwrap(), G_Y);
    }

    #[test]
    fn accepts_0x_prefix_and_whitespace() {
        let messy =
            "0x0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0001";
        assert_eq!(derive(messy, "x").unwrap(), G_X);
    }

    #[test]
    fn odd_parity_gives_03_prefix() {
        // Private key = 6 → 6·G, whose Y coordinate is odd → compressed prefix 03
        // (a published test vector: 03fff97bd5755eeea4...460297556).
        let priv_six = "0000000000000000000000000000000000000000000000000000000000000006";
        let comp = derive(priv_six, "compressed").unwrap();
        assert!(comp.starts_with("03"), "6G has odd Y → 03 prefix, got {comp}");
        assert_eq!(
            comp,
            "03fff97bd5755eeea420453a14355235d382f6472f8568a18b2f057a1460297556"
        );
        assert!(derive(priv_six, "all").unwrap().contains("y_parity: odd"));
    }

    #[test]
    fn wif_input_is_accepted() {
        // Mainnet compressed WIF for private key = 1.
        let wif = "KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn";
        assert_eq!(derive(wif, "x").unwrap(), G_X);
    }

    #[test]
    fn rejects_short_hex() {
        let err = derive("abcd", "all").unwrap_err();
        assert!(err.contains("64 hex chars"), "got: {err}");
    }

    #[test]
    fn rejects_zero_key() {
        let zero = "0000000000000000000000000000000000000000000000000000000000000000";
        let err = derive(zero, "all").unwrap_err();
        assert!(err.contains("valid secp256k1 scalar"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_format() {
        let err = derive(PRIV_ONE, "base64").unwrap_err();
        assert!(err.contains("unknown format"), "got: {err}");
    }

    #[test]
    fn empty_key_is_rejected() {
        assert!(derive("   ", "all").unwrap_err().contains("required"));
    }
}
