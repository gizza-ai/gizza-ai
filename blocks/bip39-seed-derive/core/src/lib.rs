//! gizza-ai/bip39-seed-derive core — derive the BIP39 512-bit seed from an
//! EXISTING (pasted) mnemonic phrase + an optional passphrase. No wafer/
//! wasm-bindgen deps. Sibling to `bip39-mnemonic-generator`, which *creates* a
//! phrase from entropy; this one *validates* a phrase you already have and
//! stretches it into the seed.
//!
//! BIP39 (<https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki>):
//!  - a valid mnemonic is 12/15/18/21/24 words, every word in the 2048-word list;
//!  - the last few bits encode a SHA-256 checksum over the entropy, so a typo /
//!    wrong-order phrase is detectable;
//!  - seed = PBKDF2-HMAC-SHA512(mnemonic, "mnemonic"+passphrase, 2048, 64 bytes).
//!
//! Validation is strict: an unknown word, a bad word count, or a failing
//! checksum is a hard error (that is what wallets require before restoring).

use hmac::Hmac;
use serde::Serialize;
use sha2::{Digest, Sha256, Sha512};

/// The official BIP39 English wordlist (2048 words), REUSED from the sibling
/// `bip39-mnemonic-generator` block at build time (single source, no copy).
const WORDLIST: &str = include_str!("../../../bip39-mnemonic-generator/core/src/english.txt");

/// Valid BIP39 word counts.
pub const WORD_COUNTS: [usize; 5] = [12, 15, 18, 21, 24];

/// A validated mnemonic and the seed derived from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Seed {
    /// The normalized mnemonic actually used (lowercased, single-spaced).
    pub mnemonic: String,
    /// Number of words (12/15/18/21/24).
    pub word_count: usize,
    /// Entropy strength in bits recovered from the phrase (128–256).
    pub strength: usize,
    /// The recovered raw entropy, hex-encoded.
    pub entropy_hex: String,
    /// The optional passphrase ("25th word") mixed into the seed ("" if none).
    pub passphrase: String,
    /// BIP39 512-bit seed, hex-encoded (PBKDF2-HMAC-SHA512, 2048 iters).
    pub seed_hex: String,
}

fn words() -> Vec<&'static str> {
    WORDLIST.lines().map(|w| w.trim()).filter(|w| !w.is_empty()).collect()
}

/// Validate a pasted BIP39 mnemonic and derive its 512-bit seed.
///
/// `mnemonic` is the space-separated phrase (extra/leading/trailing whitespace
/// and mixed case are tolerated — words are lowercased and single-spaced).
/// `passphrase` is the optional BIP39 passphrase; leave it empty for none.
pub fn derive(mnemonic: &str, passphrase: &str) -> Result<Seed, String> {
    // Normalize: split on any whitespace, lowercase (the English list is all
    // lowercase ASCII, so this is also the NFKD form used by BIP39).
    let toks: Vec<String> = mnemonic.split_whitespace().map(|w| w.to_lowercase()).collect();
    if toks.is_empty() {
        return Err("mnemonic is empty — paste a 12/15/18/21/24-word BIP39 phrase".into());
    }
    let word_count = toks.len();
    if !WORD_COUNTS.contains(&word_count) {
        return Err(format!(
            "a BIP39 mnemonic must be 12, 15, 18, 21, or 24 words, got {word_count} words"
        ));
    }

    // Map each word to its 11-bit index; report the first unknown word.
    let wl = words();
    let mut bits: Vec<u8> = Vec::with_capacity(word_count * 11);
    for (pos, w) in toks.iter().enumerate() {
        let idx = wl.iter().position(|x| x == w).ok_or_else(|| {
            format!("word {} (\"{w}\") is not in the BIP39 English wordlist", pos + 1)
        })?;
        for i in (0..11).rev() {
            bits.push(((idx >> i) & 1) as u8);
        }
    }

    // Split into entropy bits + checksum bits (total = ENT * 33/32).
    let total_bits = word_count * 11;
    let ent_bits = total_bits / 33 * 32;
    let checksum_bits = total_bits - ent_bits;

    // Reassemble the entropy bytes from the leading `ent_bits`.
    let mut entropy = vec![0u8; ent_bits / 8];
    for (i, &bit) in bits[..ent_bits].iter().enumerate() {
        entropy[i / 8] |= bit << (7 - (i % 8));
    }

    // Recompute the checksum and compare against the trailing bits.
    let hash = Sha256::digest(&entropy);
    for i in 0..checksum_bits {
        let expected = (hash[i / 8] >> (7 - (i % 8))) & 1;
        if bits[ent_bits + i] != expected {
            return Err(
                "invalid BIP39 checksum — a word is likely mistyped or the words are out of order"
                    .into(),
            );
        }
    }

    let normalized = toks.join(" ");
    let seed = derive_seed(&normalized, passphrase);

    Ok(Seed {
        mnemonic: normalized,
        word_count,
        strength: ent_bits,
        entropy_hex: hex::encode(&entropy),
        passphrase: passphrase.to_string(),
        seed_hex: hex::encode(seed),
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

    // Official BIP39 test vector (Trezor): "abandon" ×11 + "about", pass "TREZOR".
    #[test]
    fn vector_abandon_about_trezor() {
        let m = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let s = derive(m, "TREZOR").unwrap();
        assert_eq!(s.word_count, 12);
        assert_eq!(s.strength, 128);
        assert_eq!(s.entropy_hex, "00000000000000000000000000000000");
        assert_eq!(
            s.seed_hex,
            "c55257c360c07c72029aebc1b53c05ed0362ada38ead3e3e9efa3708e53495531f09a6987599d18264c1e1c92f2cf141630c7a3c4ab7c81b2f001698e7463b04"
        );
    }

    // Official BIP39 test vector, 24-word all-0xff entropy, pass "TREZOR".
    #[test]
    fn vector_zoo_vote_trezor() {
        let m = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo vote";
        let s = derive(m, "TREZOR").unwrap();
        assert_eq!(s.word_count, 24);
        assert_eq!(s.strength, 256);
        assert_eq!(
            s.seed_hex,
            "dd48c104698c30cfe2b6142103248622fb7bb0ff692eebb00089b32d22484e1613912f0a5b694407be899ffd31ed3992c456cdf60f5d4564b8ba3f05a69890ad"
        );
    }

    #[test]
    fn no_passphrase_differs_from_passphrase() {
        let m = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let a = derive(m, "").unwrap();
        let b = derive(m, "TREZOR").unwrap();
        assert_eq!(a.mnemonic, b.mnemonic);
        assert_ne!(a.seed_hex, b.seed_hex);
        // No-passphrase vector seed.
        assert_eq!(
            a.seed_hex,
            "5eb00bbddcf069084889a8ab9155568165f5c453ccb85e70811aaed6f6da5fc19a5ac40b389cd370d086206dec8aa6c43daea6690f20ad3d8d48b2d2ce9e38e4"
        );
    }

    #[test]
    fn tolerates_messy_whitespace_and_case() {
        let messy = "  ABANDON   abandon\tabandon abandon abandon abandon abandon abandon abandon abandon abandon About ";
        let s = derive(messy, "").unwrap();
        assert_eq!(s.word_count, 12);
        assert!(s.mnemonic.starts_with("abandon abandon"));
    }

    #[test]
    fn rejects_unknown_word() {
        let m = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon zzzz";
        let e = derive(m, "").unwrap_err();
        assert!(e.contains("not in the BIP39 English wordlist"), "got: {e}");
    }

    #[test]
    fn rejects_bad_word_count() {
        let e = derive("abandon abandon abandon", "").unwrap_err();
        assert!(e.contains("must be 12, 15, 18, 21, or 24 words"), "got: {e}");
    }

    #[test]
    fn rejects_bad_checksum() {
        // 12 valid words but the last word breaks the checksum (about → abandon).
        let m = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon";
        let e = derive(m, "").unwrap_err();
        assert!(e.contains("invalid BIP39 checksum"), "got: {e}");
    }

    #[test]
    fn rejects_empty() {
        assert!(derive("   ", "").is_err());
    }
}
