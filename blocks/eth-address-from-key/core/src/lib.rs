//! eth-address-from-key core — derive Ethereum addresses from secp256k1 private
//! keys or public keys. Ethereum addresses are the last 20 bytes of the
//! Keccak-256 digest of the uncompressed SEC1 public key without its leading
//! `0x04` byte; EIP-55 checksum case is computed from the lowercase hex address.

use k256::elliptic_curve::sec1::{FromEncodedPoint, ToEncodedPoint};
use sha3::{Digest, Keccak256};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyType {
    Auto,
    Private,
    Public,
}

impl KeyType {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Self::Auto),
            "private" | "priv" | "secret" => Ok(Self::Private),
            "public" | "pub" => Ok(Self::Public),
            other => Err(format!(
                "unknown key_type '{other}' (use auto, private, or public)"
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    All,
    Checksum,
    Lowercase,
    NoPrefix,
    Json,
}

impl OutputFormat {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "all" => Ok(Self::All),
            "checksum" | "eip55" | "eip-55" => Ok(Self::Checksum),
            "lowercase" | "lower" => Ok(Self::Lowercase),
            "no-prefix" | "noprefix" | "bare" => Ok(Self::NoPrefix),
            "json" => Ok(Self::Json),
            other => Err(format!(
                "unknown output_format '{other}' (use all, checksum, lowercase, no-prefix, or json)"
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddressParts {
    pub checksum: String,
    pub lowercase: String,
    pub no_prefix: String,
    pub public_key_uncompressed: String,
    pub public_key_compressed: String,
}

fn clean_hex(input: &str) -> Result<String, String> {
    let cleaned: String = input
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X")
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && *c != '_' && *c != ':' && *c != '-')
        .collect();
    if cleaned.is_empty() {
        return Err(
            "key is required (32-byte private key, or compressed/uncompressed public key hex)"
                .into(),
        );
    }
    if cleaned.len() % 2 != 0 {
        return Err("hex key must have an even number of digits".into());
    }
    if !cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(
            "key must be hexadecimal; base58/WIF and mnemonics are not Ethereum keys".into(),
        );
    }
    Ok(cleaned)
}

fn public_key_from_private(bytes: &[u8]) -> Result<k256::PublicKey, String> {
    if bytes.len() != 32 {
        return Err(format!(
            "private keys must be 32 bytes (64 hex chars), got {} bytes",
            bytes.len()
        ));
    }
    let sk = k256::SecretKey::from_slice(bytes).map_err(|_| {
        "private key is not a valid secp256k1 scalar (must be non-zero and below the curve order)"
            .to_string()
    })?;
    Ok(sk.public_key())
}

fn public_key_from_public(bytes: &[u8]) -> Result<k256::PublicKey, String> {
    match bytes.len() {
        33 | 65 => {
            let point = k256::EncodedPoint::from_bytes(bytes)
                .map_err(|_| "public key is not valid SEC1 compressed/uncompressed hex".to_string())?;
            Option::<k256::PublicKey>::from(k256::PublicKey::from_encoded_point(&point))
                .ok_or_else(|| "public key point is not on secp256k1".to_string())
        }
        64 => {
            let mut sec1 = Vec::with_capacity(65);
            sec1.push(0x04);
            sec1.extend_from_slice(bytes);
            public_key_from_public(&sec1)
        }
        n => Err(format!(
            "public keys must be 33-byte compressed, 65-byte uncompressed, or 64-byte x||y hex; got {n} bytes"
        )),
    }
}

fn parse_key(input: &str, key_type: KeyType) -> Result<k256::PublicKey, String> {
    let hexed = clean_hex(input)?;
    let bytes = hex::decode(&hexed).map_err(|e| format!("invalid hex key: {e}"))?;
    match key_type {
        KeyType::Private => public_key_from_private(&bytes),
        KeyType::Public => public_key_from_public(&bytes),
        KeyType::Auto => match bytes.len() {
            32 => public_key_from_private(&bytes),
            33 | 64 | 65 => public_key_from_public(&bytes),
            n => Err(format!(
                "could not auto-detect key type from {n} bytes; use 32-byte private key or 33/64/65-byte public key"
            )),
        },
    }
}

pub fn eip55_checksum(lower_40_hex: &str) -> String {
    let lower = lower_40_hex.to_ascii_lowercase();
    let hash = Keccak256::digest(lower.as_bytes());
    let mut out = String::with_capacity(42);
    out.push_str("0x");
    for (i, ch) in lower.chars().enumerate() {
        let byte = hash[i / 2];
        let nibble = if i % 2 == 0 { byte >> 4 } else { byte & 0x0f };
        if ch.is_ascii_alphabetic() && nibble >= 8 {
            out.push(ch.to_ascii_uppercase());
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn derive_parts(input: &str, key_type: &str) -> Result<AddressParts, String> {
    let pk = parse_key(input, KeyType::parse(key_type)?)?;
    let uncompressed = pk.to_encoded_point(false);
    let compressed = pk.to_encoded_point(true);
    let pub_bytes = uncompressed.as_bytes();
    let digest = Keccak256::digest(&pub_bytes[1..]);
    let no_prefix = hex::encode(&digest[12..32]);
    let checksum = eip55_checksum(&no_prefix);
    Ok(AddressParts {
        lowercase: format!("0x{no_prefix}"),
        checksum,
        no_prefix,
        public_key_uncompressed: hex::encode(pub_bytes),
        public_key_compressed: hex::encode(compressed.as_bytes()),
    })
}

pub fn run(input: &str, key_type: &str, output_format: &str) -> Result<String, String> {
    let parts = derive_parts(input, key_type)?;
    Ok(match OutputFormat::parse(output_format)? {
        OutputFormat::Checksum => parts.checksum,
        OutputFormat::Lowercase => parts.lowercase,
        OutputFormat::NoPrefix => parts.no_prefix,
        OutputFormat::Json => format!(
            "{{\n  \"address\": \"{}\",\n  \"lowercase\": \"{}\",\n  \"no_prefix\": \"{}\",\n  \"public_key_compressed\": \"{}\",\n  \"public_key_uncompressed\": \"{}\"\n}}",
            parts.checksum,
            parts.lowercase,
            parts.no_prefix,
            parts.public_key_compressed,
            parts.public_key_uncompressed
        ),
        OutputFormat::All => format!(
            "address: {}\nlowercase: {}\nno_prefix: {}\npublic_key_compressed: {}\npublic_key_uncompressed: {}",
            parts.checksum,
            parts.lowercase,
            parts.no_prefix,
            parts.public_key_compressed,
            parts.public_key_uncompressed
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIV_ONE: &str = "0000000000000000000000000000000000000000000000000000000000000001";
    const G_UNCOMPRESSED: &str = "0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8";
    const G_COMPRESSED: &str = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
    const ADDRESS_ONE: &str = "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf";

    #[test]
    fn derives_known_address_from_private_key_one() {
        assert_eq!(run(PRIV_ONE, "private", "checksum").unwrap(), ADDRESS_ONE);
    }

    #[test]
    fn derives_same_address_from_uncompressed_and_compressed_public_key() {
        assert_eq!(
            run(G_UNCOMPRESSED, "public", "checksum").unwrap(),
            ADDRESS_ONE
        );
        assert_eq!(
            run(G_COMPRESSED, "public", "checksum").unwrap(),
            ADDRESS_ONE
        );
    }

    #[test]
    fn accepts_raw_xy_public_key_without_04_prefix() {
        assert_eq!(
            run(&G_UNCOMPRESSED[2..], "public", "checksum").unwrap(),
            ADDRESS_ONE
        );
    }

    #[test]
    fn auto_detects_key_type() {
        assert_eq!(
            run(PRIV_ONE, "auto", "no-prefix").unwrap(),
            "7e5f4552091a69125d5dfcb7b8c2659029395bdf"
        );
        assert_eq!(
            run(G_COMPRESSED, "auto", "lowercase").unwrap(),
            "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf"
        );
    }

    #[test]
    fn all_output_includes_public_keys() {
        let out = run(PRIV_ONE, "auto", "all").unwrap();
        assert!(out.contains(ADDRESS_ONE));
        assert!(out.contains(G_COMPRESSED));
        assert!(out.contains(G_UNCOMPRESSED));
    }

    #[test]
    fn json_output_is_stable() {
        let out = run(PRIV_ONE, "auto", "json").unwrap();
        assert!(out.contains("\"address\": \"0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf\""));
        assert!(out.contains("\"no_prefix\": \"7e5f4552091a69125d5dfcb7b8c2659029395bdf\""));
    }

    #[test]
    fn checksum_vector_from_eip55() {
        assert_eq!(
            eip55_checksum("52908400098527886e0f7030069857d2e4169ee7"),
            "0x52908400098527886E0F7030069857D2E4169EE7"
        );
    }

    #[test]
    fn rejects_bad_key_type_and_format() {
        assert!(run(PRIV_ONE, "wif", "checksum")
            .unwrap_err()
            .contains("unknown key_type"));
        assert!(run(PRIV_ONE, "private", "csv")
            .unwrap_err()
            .contains("unknown output_format"));
    }

    #[test]
    fn rejects_invalid_lengths_and_zero_private_key() {
        assert!(run("abcd", "private", "checksum")
            .unwrap_err()
            .contains("32 bytes"));
        let zero = "0000000000000000000000000000000000000000000000000000000000000000";
        assert!(run(zero, "private", "checksum")
            .unwrap_err()
            .contains("valid secp256k1 scalar"));
    }
}
