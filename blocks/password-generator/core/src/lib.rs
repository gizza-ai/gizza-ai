//! gizza-ai/password-generator core — generate strong random passwords or
//! passphrases. No wafer/wasm-bindgen deps. Randomness via `getrandom` (WASI
//! `random_get` on wasm32-wasip1 — instantiates in the wafer runtime; the page's
//! wasm32-unknown-unknown build uses getrandom's `js` backend). Uniform indices
//! via rejection sampling (no modulo bias).

const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &str = "0123456789";
const SYMBOLS: &str = "!@#$%^&*()-_=+[]{};:,.?";

/// ~120 short, common words for passphrases (~6.9 bits/word).
const WORDS: &[&str] = &[
    "able", "acid", "aged", "also", "area", "army", "away", "baby", "back", "ball",
    "band", "bank", "base", "bath", "bear", "beat", "been", "beer", "bell", "belt",
    "best", "bird", "blue", "boat", "body", "bone", "book", "boot", "born", "boss",
    "both", "bowl", "bulk", "burn", "bush", "busy", "cake", "call", "calm", "came",
    "camp", "card", "care", "case", "cash", "cast", "cell", "chat", "chip", "city",
    "club", "coal", "coat", "code", "cold", "come", "cook", "cool", "cope", "copy",
    "core", "corn", "cost", "crew", "crop", "dark", "data", "date", "dawn", "days",
    "dead", "deal", "dear", "debt", "deep", "deny", "desk", "dial", "diet", "disk",
    "door", "dose", "down", "draw", "drop", "drug", "dual", "duke", "dust", "duty",
    "each", "earn", "ease", "east", "easy", "edge", "else", "even", "ever", "evil",
    "exit", "face", "fact", "fail", "fair", "fall", "farm", "fast", "fear", "feed",
    "feel", "feet", "fell", "file", "fill", "film", "find", "fine", "fire", "firm",
];

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

/// Generate a random password. Lowercase is always included; the flags add the
/// other classes. Returns (password, entropy_bits).
pub fn generate_password(
    length: usize,
    uppercase: bool,
    digits: bool,
    symbols: bool,
) -> Result<(String, f64), String> {
    if length == 0 || length > 512 {
        return Err("length must be between 1 and 512".into());
    }
    let mut pool = String::from(LOWER);
    if uppercase {
        pool.push_str(UPPER);
    }
    if digits {
        pool.push_str(DIGITS);
    }
    if symbols {
        pool.push_str(SYMBOLS);
    }
    let chars: Vec<char> = pool.chars().collect();
    let mut out = String::with_capacity(length);
    for _ in 0..length {
        out.push(chars[rand_index(chars.len())?]);
    }
    let bits = length as f64 * (chars.len() as f64).log2();
    Ok((out, (bits * 100.0).round() / 100.0))
}

/// Generate a random passphrase of `words` words joined by `separator`.
pub fn generate_passphrase(words: usize, separator: &str) -> Result<(String, f64), String> {
    if words == 0 || words > 20 {
        return Err("words must be between 1 and 20".into());
    }
    let mut picked = Vec::with_capacity(words);
    for _ in 0..words {
        picked.push(WORDS[rand_index(WORDS.len())?]);
    }
    let bits = words as f64 * (WORDS.len() as f64).log2();
    Ok((picked.join(separator), (bits * 100.0).round() / 100.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_length_and_charset() {
        let (p, bits) = generate_password(20, true, true, true).unwrap();
        assert_eq!(p.chars().count(), 20);
        let allowed: Vec<char> =
            format!("{LOWER}{UPPER}{DIGITS}{SYMBOLS}").chars().collect();
        assert!(p.chars().all(|c| allowed.contains(&c)));
        assert!(bits > 100.0, "20 chars over ~88-symbol pool should be >100 bits, got {bits}");
    }

    #[test]
    fn lowercase_only_when_flags_off() {
        let (p, _) = generate_password(40, false, false, false).unwrap();
        assert!(p.chars().all(|c| c.is_ascii_lowercase()), "got {p}");
    }

    #[test]
    fn two_passwords_differ() {
        let a = generate_password(24, true, true, true).unwrap().0;
        let b = generate_password(24, true, true, true).unwrap().0;
        assert_ne!(a, b);
    }

    #[test]
    fn passphrase_word_count_and_separator() {
        let (p, bits) = generate_passphrase(5, "-").unwrap();
        let parts: Vec<&str> = p.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert!(parts.iter().all(|w| WORDS.contains(w)));
        assert!(bits > 30.0, "got {bits}");
    }

    #[test]
    fn errors() {
        assert!(generate_password(0, true, true, true).is_err());
        assert!(generate_password(99999, true, true, true).is_err());
        assert!(generate_passphrase(0, "-").is_err());
        assert!(generate_passphrase(99, "-").is_err());
    }
}
