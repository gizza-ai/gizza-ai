//! gizza-ai/password-entropy core — estimate a password's strength in bits from
//! its character set and length, with a crack-time estimate and weakness flags
//! (too short, single character class, common password, repeated/sequential
//! patterns). No deps; pure. The entropy model is the standard
//! length × log2(pool-size) approximation (uniform-independent characters).

/// Result of analyzing a password.
#[derive(Debug, Clone, PartialEq)]
pub struct Strength {
    pub bits: f64,
    pub charset_size: u32,
    pub length: usize,
    pub rating: &'static str,
    pub warnings: Vec<String>,
    /// Rough offline-attack crack-time estimate (10^10 guesses/sec, average case).
    pub crack_time: String,
}

const COMMON: &[&str] = &[
    "password", "123456", "123456789", "12345678", "12345", "qwerty", "abc123",
    "password1", "111111", "1234567", "letmein", "admin", "welcome", "monkey",
    "dragon", "iloveyou", "sunshine", "princess", "football", "charlie", "aa123456",
    "qwerty123", "1q2w3e4r", "000000", "passw0rd", "starwars", "whatever", "trustno1",
];

fn rating_for(bits: f64) -> &'static str {
    if bits < 28.0 {
        "Very weak"
    } else if bits < 36.0 {
        "Weak"
    } else if bits < 60.0 {
        "Fair"
    } else if bits < 128.0 {
        "Strong"
    } else {
        "Very strong"
    }
}

fn human_time(secs: f64) -> String {
    if secs < 1.0 {
        "less than a second".into()
    } else if secs < 60.0 {
        format!("{:.0} seconds", secs)
    } else if secs < 3600.0 {
        format!("{:.0} minutes", secs / 60.0)
    } else if secs < 86_400.0 {
        format!("{:.0} hours", secs / 3600.0)
    } else if secs < 86_400.0 * 365.0 {
        format!("{:.0} days", secs / 86_400.0)
    } else {
        let years = secs / 86_400.0 / 365.0;
        if years >= 1e9 {
            "billions of years".into()
        } else if years >= 1e6 {
            "millions of years".into()
        } else {
            format!("{:.0} years", years)
        }
    }
}

/// True if `s` has an ascending or descending run of length >= 4 (by codepoint),
/// e.g. "1234", "abcd", "dcba".
fn has_sequential_run(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 4 {
        return false;
    }
    let mut asc = 1;
    let mut desc = 1;
    for w in chars.windows(2) {
        let d = w[1] as i32 - w[0] as i32;
        asc = if d == 1 { asc + 1 } else { 1 };
        desc = if d == -1 { desc + 1 } else { 1 };
        if asc >= 4 || desc >= 4 {
            return true;
        }
    }
    false
}

/// Analyze `password`.
pub fn analyze(password: &str) -> Result<Strength, String> {
    if password.is_empty() {
        return Err("password is empty".into());
    }
    let length = password.chars().count();

    let mut has_lower = false;
    let mut has_upper = false;
    let mut has_digit = false;
    let mut has_symbol = false;
    let mut has_space = false;
    let mut has_other = false;
    for c in password.chars() {
        if c.is_ascii_lowercase() {
            has_lower = true;
        } else if c.is_ascii_uppercase() {
            has_upper = true;
        } else if c.is_ascii_digit() {
            has_digit = true;
        } else if c == ' ' {
            has_space = true;
        } else if c.is_ascii_graphic() {
            has_symbol = true;
        } else {
            has_other = true;
        }
    }
    let mut pool = 0u32;
    if has_lower {
        pool += 26;
    }
    if has_upper {
        pool += 26;
    }
    if has_digit {
        pool += 10;
    }
    if has_symbol {
        pool += 33;
    }
    if has_space {
        pool += 1;
    }
    if has_other {
        pool += 100; // generous estimate for non-ASCII
    }
    let pool = pool.max(1);

    let bits = length as f64 * (pool as f64).log2();

    // Crack time: 2^bits guesses at 1e10/sec, average case = half.
    let crack_time = if bits >= 200.0 {
        "centuries (effectively uncrackable)".to_string()
    } else {
        human_time(2f64.powf(bits) / 1e10 / 2.0)
    };

    let classes = [has_lower, has_upper, has_digit, has_symbol, has_space, has_other]
        .iter()
        .filter(|&&b| b)
        .count();

    let mut warnings = Vec::new();
    if length < 8 {
        warnings.push("shorter than 8 characters".to_string());
    }
    if classes <= 1 {
        warnings.push("uses only one type of character".to_string());
    }
    let lower = password.to_lowercase();
    if COMMON.contains(&lower.as_str()) {
        warnings.push("is a very common password".to_string());
    } else if COMMON.iter().any(|c| lower.contains(c)) {
        warnings.push("contains a very common password".to_string());
    }
    if length >= 3 && password.chars().all(|c| Some(c) == password.chars().next()) {
        warnings.push("is a single repeated character".to_string());
    }
    if has_sequential_run(password) {
        warnings.push("contains a sequential run (like 1234 or abcd)".to_string());
    }

    Ok(Strength {
        bits: (bits * 100.0).round() / 100.0,
        charset_size: pool,
        length,
        rating: rating_for(bits),
        warnings,
        crack_time,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charset_and_bits_basic() {
        let s = analyze("abc").unwrap();
        assert_eq!(s.charset_size, 26);
        assert_eq!(s.length, 3);
        // 3 * log2(26) ≈ 14.10
        assert!((s.bits - 14.10).abs() < 0.1, "got {}", s.bits);
        assert_eq!(s.rating, "Very weak");
    }

    #[test]
    fn mixed_charset_pool() {
        let s = analyze("Ab1!").unwrap();
        // 26+26+10+33 = 95
        assert_eq!(s.charset_size, 95);
    }

    #[test]
    fn common_password_flagged() {
        let s = analyze("password").unwrap();
        assert!(s.warnings.iter().any(|w| w.contains("common password")), "{:?}", s.warnings);
    }

    #[test]
    fn repeated_char_flagged() {
        let s = analyze("aaaaaaaa").unwrap();
        assert!(s.warnings.iter().any(|w| w.contains("repeated character")));
        assert!(s.warnings.iter().any(|w| w.contains("one type")));
    }

    #[test]
    fn sequential_flagged() {
        assert!(analyze("abcd9").unwrap().warnings.iter().any(|w| w.contains("sequential")));
        assert!(analyze("x1234y").unwrap().warnings.iter().any(|w| w.contains("sequential")));
        assert!(!analyze("a1b2c3d4").unwrap().warnings.iter().any(|w| w.contains("sequential")));
    }

    #[test]
    fn strong_password_rates_high_no_warnings() {
        let s = analyze("8#kQ!v2pL@9zR4nW").unwrap();
        assert!(s.bits >= 90.0, "got {}", s.bits);
        assert!(matches!(s.rating, "Strong" | "Very strong"));
        assert!(s.warnings.is_empty(), "{:?}", s.warnings);
        assert!(!s.crack_time.is_empty());
    }

    #[test]
    fn errors_on_empty() {
        assert!(analyze("").is_err());
    }
}
