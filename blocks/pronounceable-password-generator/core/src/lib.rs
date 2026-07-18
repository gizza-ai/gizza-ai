//! gizza-ai/pronounceable-password-generator core — build easy-to-say, easy-to-type
//! passwords from consonant/vowel phoneme patterns, then inject digits and symbols.
//! No wafer/wasm-bindgen deps. Randomness via `getrandom` (WASI `random_get` on
//! wasm32-wasip1 — instantiates in the wafer runtime; the page's
//! wasm32-unknown-unknown build uses getrandom's `js` backend). Uniform indices
//! via rejection sampling (no modulo bias).

/// Pronounceable consonants — drops `q` (needs `u`), `x`, and `y` to avoid awkward
/// clusters. 18 letters.
const CONSONANTS: &str = "bcdfghjklmnprstvwz";
/// Vowels. 5 letters.
const VOWELS: &str = "aeiou";
/// Readable symbol set injected at the end.
const SYMBOLS: &str = "!@#$%&*?-_+=";
const DIGITS: &str = "0123456789";

/// Uniform random index in `0..n` via rejection sampling (no modulo bias).
fn rand_index(n: usize) -> Result<usize, String> {
    if n == 0 {
        return Err("empty selection set".into());
    }
    let nn = n as u64;
    let zone = (u64::from(u32::MAX) + 1) / nn * nn; // largest multiple of n <= 2^32
    loop {
        let mut b = [0u8; 4];
        getrandom::getrandom(&mut b).map_err(|e| format!("RNG error: {e}"))?;
        let v = u64::from(u32::from_le_bytes(b));
        if v < zone {
            return Ok((v % nn) as usize);
        }
    }
}

/// Generate a pronounceable password.
///
/// The pronounceable core is `length` letters that strictly alternate consonant /
/// vowel (starting with a consonant), so there are never unpronounceable clusters —
/// e.g. `bofuka`. `digits` random digits and `symbols` random symbols are then
/// appended (a two-digit suffix is a common, readability-preserving way to add
/// entropy). With `capitalize`, the first letter is upper-cased.
///
/// Returns `(password, entropy_bits)`. Entropy is summed over the actual random
/// choices — the consonant slots (18 options), vowel slots (5), each digit (10),
/// and each symbol (12); capitalization is deterministic and adds none.
pub fn generate_pronounceable(
    length: usize,
    capitalize: bool,
    digits: usize,
    symbols: usize,
) -> Result<(String, f64), String> {
    if !(4..=64).contains(&length) {
        return Err("length must be between 4 and 64".into());
    }
    if digits > 12 {
        return Err("digits must be between 0 and 12".into());
    }
    if symbols > 12 {
        return Err("symbols must be between 0 and 12".into());
    }
    let cons: Vec<char> = CONSONANTS.chars().collect();
    let vows: Vec<char> = VOWELS.chars().collect();
    let mut out = String::with_capacity(length + digits + symbols);
    let mut bits = 0.0f64;

    for i in 0..length {
        let set = if i % 2 == 0 { &cons } else { &vows };
        let mut ch = set[rand_index(set.len())?];
        if capitalize && i == 0 {
            ch = ch.to_ascii_uppercase();
        }
        out.push(ch);
        bits += (set.len() as f64).log2();
    }

    let dg: Vec<char> = DIGITS.chars().collect();
    for _ in 0..digits {
        out.push(dg[rand_index(dg.len())?]);
        bits += (dg.len() as f64).log2();
    }

    let sy: Vec<char> = SYMBOLS.chars().collect();
    for _ in 0..symbols {
        out.push(sy[rand_index(sy.len())?]);
        bits += (sy.len() as f64).log2();
    }

    Ok((out, (bits * 100.0).round() / 100.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_default_shape() {
        let (pw, bits) = generate_pronounceable(12, true, 2, 1).unwrap();
        // 12 letters + 2 digits + 1 symbol.
        assert_eq!(pw.chars().count(), 15);
        // First letter capitalized.
        assert!(pw.chars().next().unwrap().is_ascii_uppercase());
        // Letters strictly alternate consonant / vowel (case-insensitively).
        let letters: Vec<char> = pw.chars().take(12).map(|c| c.to_ascii_lowercase()).collect();
        for (i, c) in letters.iter().enumerate() {
            if i % 2 == 0 {
                assert!(CONSONANTS.contains(*c), "position {i} ({c}) should be a consonant");
            } else {
                assert!(VOWELS.contains(*c), "position {i} ({c}) should be a vowel");
            }
        }
        // Trailing 2 digits + 1 symbol.
        let tail: Vec<char> = pw.chars().skip(12).collect();
        assert!(tail[0].is_ascii_digit() && tail[1].is_ascii_digit());
        assert!(SYMBOLS.contains(tail[2]));
        // Entropy: 6*log2(18) + 6*log2(5) + 2*log2(10) + log2(12) ≈ 49.18 bits.
        assert!((bits - 49.18).abs() < 0.1, "unexpected entropy {bits}");
    }

    #[test]
    fn no_capitalize_starts_lowercase() {
        let (pw, _) = generate_pronounceable(6, false, 0, 0).unwrap();
        assert_eq!(pw.chars().count(), 6);
        assert!(pw.chars().next().unwrap().is_ascii_lowercase());
    }

    #[test]
    fn rejects_short_length() {
        assert!(generate_pronounceable(3, true, 2, 0).is_err());
    }

    #[test]
    fn rejects_too_many_symbols() {
        assert!(generate_pronounceable(10, true, 0, 13).is_err());
    }
}
