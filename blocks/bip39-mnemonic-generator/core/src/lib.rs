//! gizza-ai/bip39-mnemonic-generator core — generate a BIP39 mnemonic seed
//! phrase from secure entropy (or from supplied entropy, deterministically) and
//! derive the BIP39 512-bit seed. No wafer/wasm-bindgen deps.
//!
//! BIP39 (<https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki>):
//!  1. Take ENT bits of entropy (128/160/192/224/256 → 12/15/18/21/24 words).
//!  2. checksum = first (ENT/32) bits of SHA-256(entropy).
//!  3. Split entropy||checksum into 11-bit groups; each indexes the 2048-word list.
//!  4. seed = PBKDF2-HMAC-SHA512(mnemonic, "mnemonic"+passphrase, 2048, 64 bytes).
//!
//! The CSPRNG is `getrandom` (WASI `random_get` on wasm32-wasip1 / JS crypto on
//! the page), so random generation runs on every backend. Supplying explicit
//! `entropy_hex` makes the whole thing deterministic (handy for the page test,
//! recovery, and BIP39 test vectors).

use hmac::Hmac;
use serde::Serialize;
use sha2::{Digest, Sha256, Sha512};

/// The official BIP39 English wordlist (2048 words), embedded at build time.
const WORDLIST: &str = include_str!("english.txt");

/// A generated / recovered BIP39 mnemonic and the derived seed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mnemonic {
    /// Space-separated mnemonic words.
    pub mnemonic: String,
    /// Number of words (12/15/18/21/24).
    pub word_count: usize,
    /// Entropy strength in bits (128/160/192/224/256).
    pub strength: usize,
    /// The raw entropy, hex-encoded.
    pub entropy_hex: String,
    /// BIP39 512-bit seed, hex-encoded (PBKDF2-HMAC-SHA512, 2048 iters).
    pub seed_hex: String,
    /// The optional passphrase ("25th word") mixed into the seed ("" if none).
    pub passphrase: String,
}

/// All BIP39-valid entropy strengths (bits) and their word counts.
pub const STRENGTHS: [usize; 5] = [128, 160, 192, 224, 256];

fn words() -> Vec<&'static str> {
    WORDLIST.lines().map(|w| w.trim()).filter(|w| !w.is_empty()).collect()
}

/// Generate a fresh mnemonic from a secure RNG.
///
/// `strength` is the entropy size in bits — one of 128/160/192/224/256 (other
/// values are rejected). `passphrase` is the optional BIP39 passphrase.
pub fn generate(strength: usize, passphrase: &str) -> Result<Mnemonic, String> {
    if !STRENGTHS.contains(&strength) {
        return Err(format!(
            "strength must be one of 128, 160, 192, 224, 256 bits, got {strength}"
        ));
    }
    let mut entropy = vec![0u8; strength / 8];
    getrandom::getrandom(&mut entropy).map_err(|e| format!("RNG failure: {e}"))?;
    from_entropy(&entropy, passphrase)
}

/// Build a mnemonic from explicit entropy (deterministic). `entropy_hex` must be
/// a hex string of 16/20/24/28/32 bytes (128–256 bits, a multiple of 32 bits).
pub fn from_entropy_hex(entropy_hex: &str, passphrase: &str) -> Result<Mnemonic, String> {
    let cleaned: String = entropy_hex.split_whitespace().collect();
    let entropy = hex::decode(cleaned.trim())
        .map_err(|e| format!("entropy must be valid hex: {e}"))?;
    from_entropy(&entropy, passphrase)
}

/// Core: entropy bytes → mnemonic + seed.
pub fn from_entropy(entropy: &[u8], passphrase: &str) -> Result<Mnemonic, String> {
    let ent_bits = entropy.len() * 8;
    if !STRENGTHS.contains(&ent_bits) {
        return Err(format!(
            "entropy must be 16/20/24/28/32 bytes (128–256 bits, multiple of 32), got {} bytes ({ent_bits} bits)",
            entropy.len()
        ));
    }
    let checksum_bits = ent_bits / 32;

    // Build the bit string: entropy bits, then the leading `checksum_bits` of SHA-256(entropy).
    let hash = Sha256::digest(entropy);
    let total_bits = ent_bits + checksum_bits;
    let mut bits = Vec::with_capacity(total_bits);
    for &b in entropy {
        for i in (0..8).rev() {
            bits.push((b >> i) & 1);
        }
    }
    for i in 0..checksum_bits {
        let byte = hash[i / 8];
        let bit = (byte >> (7 - (i % 8))) & 1;
        bits.push(bit);
    }

    let wl = words();
    let mut out = Vec::with_capacity(total_bits / 11);
    for group in bits.chunks(11) {
        let mut idx = 0usize;
        for &bit in group {
            idx = (idx << 1) | bit as usize;
        }
        out.push(wl[idx]);
    }
    let mnemonic = out.join(" ");
    let seed = derive_seed(&mnemonic, passphrase);

    Ok(Mnemonic {
        mnemonic,
        word_count: out.len(),
        strength: ent_bits,
        entropy_hex: hex::encode(entropy),
        seed_hex: hex::encode(seed),
        passphrase: passphrase.to_string(),
    })
}

/// BIP39 seed = PBKDF2-HMAC-SHA512(mnemonic, "mnemonic"+passphrase, 2048, 64 bytes).
fn derive_seed(mnemonic: &str, passphrase: &str) -> [u8; 64] {
    let salt = format!("mnemonic{passphrase}");
    let mut seed = [0u8; 64];
    pbkdf2::pbkdf2::<Hmac<Sha512>>(mnemonic.as_bytes(), salt.as_bytes(), 2048, &mut seed)
        .expect("HMAC accepts any key length");
    seed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wordlist_has_2048_words() {
        assert_eq!(words().len(), 2048);
    }

    #[test]
    fn random_generate_12_words() {
        let m = generate(128, "").unwrap();
        assert_eq!(m.word_count, 12);
        assert_eq!(m.strength, 128);
        assert_eq!(m.entropy_hex.len(), 32); // 16 bytes
        assert_eq!(m.seed_hex.len(), 128); // 64 bytes
        // Every word is in the list.
        for w in m.mnemonic.split(' ') {
            assert!(words().contains(&w), "unknown word {w}");
        }
    }

    #[test]
    fn random_generate_24_words() {
        let m = generate(256, "").unwrap();
        assert_eq!(m.word_count, 24);
    }

    #[test]
    fn rejects_bad_strength() {
        assert!(generate(100, "").is_err());
    }

    #[test]
    fn rejects_bad_entropy_len() {
        assert!(from_entropy(&[0u8; 10], "").is_err());
    }

    // Official BIP39 test vector (Trezor), entropy = all-zero 128 bits.
    // https://github.com/trezor/python-mnemonic/blob/master/vectors.json
    #[test]
    fn vector_all_zero_128() {
        let m = from_entropy_hex("00000000000000000000000000000000", "TREZOR").unwrap();
        assert_eq!(
            m.mnemonic,
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        );
        assert_eq!(
            m.seed_hex,
            "c55257c360c07c72029aebc1b53c05ed0362ada38ead3e3e9efa3708e53495531f09a6987599d18264c1e1c92f2cf141630c7a3c4ab7c81b2f001698e7463b04"
        );
    }

    // Official BIP39 test vector, 256-bit all-0xff entropy.
    #[test]
    fn vector_all_ff_256() {
        let m = from_entropy_hex(
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            "TREZOR",
        )
        .unwrap();
        assert_eq!(m.word_count, 24);
        assert_eq!(
            m.mnemonic,
            "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo vote"
        );
        assert_eq!(
            m.seed_hex,
            "dd48c104698c30cfe2b6142103248622fb7bb0ff692eebb00089b32d22484e1613912f0a5b694407be899ffd31ed3992c456cdf60f5d4564b8ba3f05a69890ad"
        );
    }

    #[test]
    fn deterministic_from_same_entropy() {
        let a = from_entropy_hex("0c1e24e5917779d297e14d45f14e1a1a", "").unwrap();
        let b = from_entropy_hex("0c1e24e5917779d297e14d45f14e1a1a", "").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn passphrase_changes_seed() {
        let a = from_entropy_hex("00000000000000000000000000000000", "").unwrap();
        let b = from_entropy_hex("00000000000000000000000000000000", "x").unwrap();
        assert_eq!(a.mnemonic, b.mnemonic);
        assert_ne!(a.seed_hex, b.seed_hex);
    }
}
