//! gizza-ai/crypto-keypair-generator core — generate a fresh keypair and wallet
//! address for a blockchain, fully offline. No wafer/wasm-bindgen deps.
//!
//! * **Bitcoin / Ethereum** — secp256k1 via pure-Rust `k256`.
//!   * Bitcoin: compressed-pubkey legacy **P2PKH** address (`1…`, base58check,
//!     mainnet) + **WIF** private key.
//!   * Ethereum: Keccak-256 of the 64-byte uncompressed public key, last 20
//!     bytes, **EIP-55** mixed-case checksummed hex.
//! * **Solana** — Ed25519 via `ed25519-dalek`; address is base58 of the 32-byte
//!   public key; the private export is base58 of the 64-byte secret‖public
//!   keypair (the Solana CLI / wallet format).
//!
//! The CSPRNG is `getrandom` (WASI `random_get` on wasm32-wasip1), so this runs
//! on every backend. Address encodings are covered by known-answer tests
//! (EIP-55 spec vectors, the private-key=1 Ethereum vector, a base58check
//! vector, and decode round-trips).

use k256::elliptic_curve::sec1::ToEncodedPoint;
use ripemd::Ripemd160;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sha3::Keccak256;

/// A supported blockchain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chain {
    Bitcoin,
    Ethereum,
    Solana,
}

impl Chain {
    /// Parse a chain name; accepts the full name or the ticker.
    pub fn parse(s: &str) -> Result<Chain, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "bitcoin" | "btc" => Ok(Chain::Bitcoin),
            "ethereum" | "eth" => Ok(Chain::Ethereum),
            "solana" | "sol" => Ok(Chain::Solana),
            other => Err(format!(
                "unsupported chain {other:?} (expected bitcoin, ethereum, or solana)"
            )),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Chain::Bitcoin => "bitcoin",
            Chain::Ethereum => "ethereum",
            Chain::Solana => "solana",
        }
    }
}

/// A generated keypair with a chain-native wallet address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyPair {
    pub chain: String,
    /// Raw 32-byte private scalar / seed, lower-hex.
    pub private_key_hex: String,
    /// Public key hex: compressed secp256k1 (33 B) for BTC, uncompressed
    /// (65 B, `0x04`-prefixed) for ETH, 32-byte Ed25519 point for SOL.
    pub public_key_hex: String,
    /// The wallet address in the chain's canonical text encoding.
    pub address: String,
    /// Chain-native private-key export: Bitcoin WIF (compressed), Ethereum
    /// `0x`-hex, Solana base58 of the 64-byte secret‖public keypair.
    pub private_key_export: String,
    /// One-line description of the address format.
    pub address_format: String,
}

/// Generate a fresh keypair for `chain`, drawing from the OS CSPRNG.
pub fn generate(chain: Chain) -> KeyPair {
    match chain {
        Chain::Bitcoin => bitcoin_from_secret(&random_secp256k1_scalar()),
        Chain::Ethereum => ethereum_from_secret(&random_secp256k1_scalar()),
        Chain::Solana => solana_from_seed(&random_32()),
    }
}

/// A uniformly-random valid secp256k1 scalar (non-zero, < curve order), drawn
/// via k256's own rejection sampling, exported as 32 big-endian bytes.
fn random_secp256k1_scalar() -> [u8; 32] {
    let sk = k256::SecretKey::random(&mut rand::rngs::OsRng);
    let mut out = [0u8; 32];
    out.copy_from_slice(&sk.to_bytes());
    out
}

fn random_32() -> [u8; 32] {
    use rand::RngCore;
    let mut b = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut b);
    b
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// base58check: append the first 4 bytes of double-SHA-256 as a checksum, then
/// base58-encode. Used for Bitcoin addresses and WIF private keys.
fn base58check(payload: &[u8]) -> String {
    let checksum = Sha256::digest(Sha256::digest(payload));
    let mut full = payload.to_vec();
    full.extend_from_slice(&checksum[..4]);
    bs58::encode(full).into_string()
}

fn bitcoin_from_secret(sk_bytes: &[u8; 32]) -> KeyPair {
    let sk = k256::SecretKey::from_slice(sk_bytes).expect("valid 32-byte secp256k1 scalar");
    let pk_point = sk.public_key().to_encoded_point(true); // compressed, 33 bytes
    let pk = pk_point.as_bytes();

    // hash160 = RIPEMD-160(SHA-256(pubkey))
    let h160 = Ripemd160::digest(Sha256::digest(pk));
    let mut addr_payload = Vec::with_capacity(21);
    addr_payload.push(0x00); // mainnet P2PKH version byte
    addr_payload.extend_from_slice(&h160);
    let address = base58check(&addr_payload);

    // WIF: version 0x80 ‖ secret ‖ 0x01 (compressed-pubkey flag)
    let mut wif_payload = Vec::with_capacity(34);
    wif_payload.push(0x80);
    wif_payload.extend_from_slice(sk_bytes);
    wif_payload.push(0x01);
    let wif = base58check(&wif_payload);

    KeyPair {
        chain: Chain::Bitcoin.label().to_string(),
        private_key_hex: hex(sk_bytes),
        public_key_hex: hex(pk),
        address,
        private_key_export: wif,
        address_format: "P2PKH legacy address (base58check, mainnet), compressed public key".into(),
    }
}

fn ethereum_from_secret(sk_bytes: &[u8; 32]) -> KeyPair {
    let sk = k256::SecretKey::from_slice(sk_bytes).expect("valid 32-byte secp256k1 scalar");
    let pk_point = sk.public_key().to_encoded_point(false); // uncompressed: 0x04 ‖ X ‖ Y
    let pk = pk_point.as_bytes();
    let hash = Keccak256::digest(&pk[1..]); // hash the 64-byte X‖Y (drop the 0x04 tag)
    let address = eip55(&hash[12..32]);

    KeyPair {
        chain: Chain::Ethereum.label().to_string(),
        private_key_hex: hex(sk_bytes),
        public_key_hex: format!("0x{}", hex(pk)),
        address,
        private_key_export: format!("0x{}", hex(sk_bytes)),
        address_format: "EIP-55 checksummed hex address".into(),
    }
}

/// EIP-55 mixed-case checksum of a 20-byte address.
fn eip55(addr: &[u8]) -> String {
    let lower = hex(addr); // 40 lowercase hex chars
    let hash = Keccak256::digest(lower.as_bytes());
    let mut out = String::with_capacity(42);
    out.push_str("0x");
    for (i, ch) in lower.chars().enumerate() {
        if ch.is_ascii_digit() {
            out.push(ch);
        } else {
            let nibble = if i % 2 == 0 { hash[i / 2] >> 4 } else { hash[i / 2] & 0x0f };
            out.push(if nibble >= 8 { ch.to_ascii_uppercase() } else { ch });
        }
    }
    out
}

fn solana_from_seed(seed: &[u8; 32]) -> KeyPair {
    let sk = ed25519_dalek::SigningKey::from_bytes(seed);
    let vk = sk.verifying_key();
    let pk = vk.to_bytes();
    let address = bs58::encode(pk).into_string();

    // Solana wallet keypair export = base58(secret ‖ public), 64 bytes.
    let mut keypair = Vec::with_capacity(64);
    keypair.extend_from_slice(seed);
    keypair.extend_from_slice(&pk);
    let secret58 = bs58::encode(keypair).into_string();

    KeyPair {
        chain: Chain::Solana.label().to_string(),
        private_key_hex: hex(seed),
        public_key_hex: hex(&pk),
        address,
        private_key_export: secret58,
        address_format: "base58 Ed25519 public key".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret_from_hex(h: &str) -> [u8; 32] {
        let bytes: Vec<u8> = (0..h.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
            .collect();
        let mut out = [0u8; 32];
        out.copy_from_slice(&bytes);
        out
    }

    #[test]
    fn chain_parse_accepts_names_and_tickers() {
        assert_eq!(Chain::parse("bitcoin").unwrap(), Chain::Bitcoin);
        assert_eq!(Chain::parse("BTC").unwrap(), Chain::Bitcoin);
        assert_eq!(Chain::parse(" Ethereum ").unwrap(), Chain::Ethereum);
        assert_eq!(Chain::parse("sol").unwrap(), Chain::Solana);
        assert!(Chain::parse("dogecoin").is_err());
    }

    // EIP-55 canonical test vectors (from the EIP-55 specification).
    #[test]
    fn eip55_matches_spec_vectors() {
        for expected in [
            "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
            "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359",
            "0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB",
            "0xD1220A0cf47c7B9Be7A2E6BA89F429762e7b9aDb",
        ] {
            let raw: Vec<u8> = (2..expected.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&expected[i..i + 2], 16).unwrap())
                .collect();
            assert_eq!(eip55(&raw), expected);
        }
    }

    // Well-known vector: Ethereum private key = 1 → 0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf.
    // Proves the k256 secp256k1 public-key derivation + Keccak-256 + EIP-55 path.
    #[test]
    fn ethereum_private_key_one_known_address() {
        let kp = ethereum_from_secret(&secret_from_hex(
            "0000000000000000000000000000000000000000000000000000000000000001",
        ));
        assert_eq!(kp.address, "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf");
        // Uncompressed pubkey for privkey=1 begins with 0x04 then the generator point X (79be667e…).
        assert!(kp.public_key_hex.starts_with("0x0479be667ef9dcbbac55a06295ce870b"));
    }

    // base58check vector: version 0x00 + 20 zero bytes = the canonical all-zero
    // P2PKH address. Proves base58check (double-SHA-256 checksum + base58).
    #[test]
    fn base58check_all_zero_p2pkh() {
        let payload = [0u8; 21]; // version 0x00 + 20 zero bytes
        assert_eq!(base58check(&payload), "1111111111111111111114oLvT2");
    }

    // Bitcoin: privkey=1 yields the secp256k1 generator point as the compressed
    // pubkey, and a well-formed P2PKH address that decodes back to
    // version(0x00) ‖ hash160(pubkey).
    #[test]
    fn bitcoin_private_key_one_structure() {
        let kp = bitcoin_from_secret(&secret_from_hex(
            "0000000000000000000000000000000000000000000000000000000000000001",
        ));
        // Compressed generator point.
        assert_eq!(
            kp.public_key_hex,
            "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
        );
        assert!(kp.address.starts_with('1'));
        // Decode the address and confirm it re-hashes correctly.
        let decoded = bs58::decode(&kp.address).into_vec().unwrap();
        assert_eq!(decoded[0], 0x00); // version
        let expected_h160 = Ripemd160::digest(Sha256::digest(
            hex_to_bytes(&kp.public_key_hex),
        ));
        assert_eq!(&decoded[1..21], expected_h160.as_slice());
        // WIF for a compressed key starts with 'K' or 'L'.
        assert!(kp.private_key_export.starts_with('K') || kp.private_key_export.starts_with('L'));
    }

    #[test]
    fn solana_seed_round_trips_to_address() {
        let seed = secret_from_hex(
            "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20",
        );
        let kp = solana_from_seed(&seed);
        // Address base58-decodes to the 32-byte public key.
        let decoded = bs58::decode(&kp.address).into_vec().unwrap();
        assert_eq!(decoded, hex_to_bytes(&kp.public_key_hex));
        // Private export decodes to 64 bytes = seed ‖ public.
        let kb = bs58::decode(&kp.private_key_export).into_vec().unwrap();
        assert_eq!(kb.len(), 64);
        assert_eq!(&kb[..32], &seed);
        assert_eq!(&kb[32..], decoded.as_slice());
    }

    #[test]
    fn generate_produces_distinct_valid_keypairs() {
        for chain in [Chain::Bitcoin, Chain::Ethereum, Chain::Solana] {
            let a = generate(chain);
            let b = generate(chain);
            assert_ne!(a.private_key_hex, b.private_key_hex, "keys must be random");
            assert_eq!(a.private_key_hex.len(), 64);
            assert!(!a.address.is_empty());
            assert_eq!(a.chain, chain.label());
        }
    }

    fn hex_to_bytes(h: &str) -> Vec<u8> {
        let h = h.trim_start_matches("0x");
        (0..h.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
            .collect()
    }
}
