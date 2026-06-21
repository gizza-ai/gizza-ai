//! gizza-ai/random-token-generator core — generate cryptographically random
//! tokens with a configurable length and character set. No wafer/wasm-bindgen
//! deps. Randomness via `getrandom` (WASI `random_get` on wasm32-wasip1 —
//! instantiates in the wafer runtime; the page's wasm32-unknown-unknown build
//! uses getrandom's `js` backend). Uniform indices via rejection sampling so
//! there is no modulo bias even for non-power-of-two alphabets.

const HEX_LOWER: &str = "0123456789abcdef";
const HEX_UPPER: &str = "0123456789ABCDEF";
const NUMERIC: &str = "0123456789";
const ALPHA: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
const ALPHANUMERIC: &str =
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
// URL-safe base64 alphabet (RFC 4648 §5, no padding).
const BASE64URL: &str =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
// Crockford-style "safe" set used by many API-key generators: alphanumeric with
// the easily-confused characters (0/O, 1/l/I) removed.
const SAFE: &str = "23456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz";

/// Resolve a named character-set preset to its alphabet string.
fn preset_charset(charset: &str) -> Result<&'static str, String> {
    Ok(match charset.trim().to_ascii_lowercase().as_str() {
        "" | "hex" => HEX_LOWER,
        "hex-upper" | "hex_upper" | "hexupper" => HEX_UPPER,
        "base64url" | "base64" | "base64-url" => BASE64URL,
        "alphanumeric" | "base62" | "alnum" => ALPHANUMERIC,
        "alphabetic" | "alpha" | "letters" => ALPHA,
        "numeric" | "digits" | "decimal" => NUMERIC,
        "safe" | "no-ambiguous" | "unambiguous" => SAFE,
        other => {
            return Err(format!(
                "unknown charset {other:?} (use hex, hex-upper, base64url, alphanumeric, alphabetic, numeric, safe, or custom)"
            ))
        }
    })
}

/// Uniform random index in `0..n` via rejection sampling (no modulo bias).
fn rand_index(n: usize) -> Result<usize, String> {
    if n == 0 {
        return Err("empty character set".into());
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

/// Build the alphabet for a request. `charset` is a preset name unless
/// `custom_chars` is non-empty, in which case the unique characters of
/// `custom_chars` are used verbatim.
fn build_alphabet(charset: &str, custom_chars: &str) -> Result<Vec<char>, String> {
    let custom = custom_chars.trim();
    if !custom.is_empty() {
        let mut seen: Vec<char> = Vec::new();
        for c in custom_chars.chars() {
            if !seen.contains(&c) {
                seen.push(c);
            }
        }
        if seen.len() < 2 {
            return Err("custom_chars must contain at least 2 distinct characters".into());
        }
        return Ok(seen);
    }
    Ok(preset_charset(charset)?.chars().collect())
}

/// Generate a single random token of `length` characters drawn uniformly from
/// the resolved alphabet. Returns (token, entropy_bits).
fn one_token(length: usize, alphabet: &[char]) -> Result<(String, f64), String> {
    let mut out = String::with_capacity(length);
    for _ in 0..length {
        out.push(alphabet[rand_index(alphabet.len())?]);
    }
    let bits = length as f64 * (alphabet.len() as f64).log2();
    Ok((out, (bits * 100.0).round() / 100.0))
}

/// A generated batch: the tokens plus the per-token entropy in bits.
pub struct Tokens {
    pub tokens: Vec<String>,
    pub bits: f64,
    pub charset_size: usize,
}

/// Generate `count` random tokens, each of `length` characters from `charset`
/// (a preset name) or `custom_chars` (overrides the preset when non-empty).
pub fn generate(
    length: usize,
    count: usize,
    charset: &str,
    custom_chars: &str,
) -> Result<Tokens, String> {
    if length == 0 || length > 4096 {
        return Err("length must be between 1 and 4096".into());
    }
    if count == 0 || count > 1000 {
        return Err("count must be between 1 and 1000".into());
    }
    let alphabet = build_alphabet(charset, custom_chars)?;
    let mut tokens = Vec::with_capacity(count);
    let mut bits = 0.0;
    for _ in 0..count {
        let (t, b) = one_token(length, &alphabet)?;
        tokens.push(t);
        bits = b;
    }
    Ok(Tokens {
        tokens,
        bits,
        charset_size: alphabet.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_default_charset() {
        let t = generate(32, 1, "hex", "").unwrap();
        assert_eq!(t.tokens.len(), 1);
        assert_eq!(t.tokens[0].chars().count(), 32);
        assert!(t.tokens[0].chars().all(|c| HEX_LOWER.contains(c)), "{}", t.tokens[0]);
        assert_eq!(t.charset_size, 16);
        // 32 hex chars = 128 bits.
        assert!((t.bits - 128.0).abs() < 0.01, "got {}", t.bits);
    }

    #[test]
    fn empty_charset_falls_back_to_hex() {
        let t = generate(8, 1, "", "").unwrap();
        assert!(t.tokens[0].chars().all(|c| HEX_LOWER.contains(c)));
    }

    #[test]
    fn presets_resolve() {
        for (name, alpha) in [
            ("hex-upper", HEX_UPPER),
            ("base64url", BASE64URL),
            ("alphanumeric", ALPHANUMERIC),
            ("alphabetic", ALPHA),
            ("numeric", NUMERIC),
            ("safe", SAFE),
        ] {
            let t = generate(50, 1, name, "").unwrap();
            assert!(
                t.tokens[0].chars().all(|c| alpha.contains(c)),
                "charset {name}: {}",
                t.tokens[0]
            );
            assert_eq!(t.charset_size, alpha.chars().count());
        }
    }

    #[test]
    fn safe_set_has_no_ambiguous_chars() {
        let t = generate(200, 1, "safe", "").unwrap();
        for c in t.tokens[0].chars() {
            assert!(!matches!(c, '0' | 'O' | '1' | 'l' | 'I'), "ambiguous {c}");
        }
    }

    #[test]
    fn custom_charset_overrides_preset() {
        let t = generate(40, 1, "hex", "AB").unwrap();
        assert!(t.tokens[0].chars().all(|c| c == 'A' || c == 'B'), "{}", t.tokens[0]);
        assert_eq!(t.charset_size, 2);
    }

    #[test]
    fn custom_charset_dedups() {
        let t = generate(10, 1, "hex", "aabbcc").unwrap();
        assert_eq!(t.charset_size, 3);
    }

    #[test]
    fn count_produces_multiple_distinct_tokens() {
        let t = generate(24, 5, "base64url", "").unwrap();
        assert_eq!(t.tokens.len(), 5);
        // With 24 base64url chars (~143 bits) collisions are astronomically unlikely.
        let mut uniq = t.tokens.clone();
        uniq.sort();
        uniq.dedup();
        assert_eq!(uniq.len(), 5, "tokens should differ");
    }

    #[test]
    fn errors() {
        assert!(generate(0, 1, "hex", "").is_err());
        assert!(generate(99999, 1, "hex", "").is_err());
        assert!(generate(8, 0, "hex", "").is_err());
        assert!(generate(8, 99999, "hex", "").is_err());
        assert!(generate(8, 1, "nonsense", "").is_err());
        assert!(generate(8, 1, "hex", "x").is_err(), "1-char custom set rejected");
    }
}
