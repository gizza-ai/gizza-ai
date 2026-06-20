//! gizza-ai/luhn-validate core — validate a number with the Luhn (mod-10)
//! check-digit algorithm and report the would-be check digit + a best-effort
//! card brand. No deps; pure. Spaces and dashes in the input are ignored.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuhnResult {
    /// True if the number passes the Luhn check.
    pub valid: bool,
    /// The cleaned digit string (spaces/dashes removed).
    pub digits: String,
    pub length: usize,
    /// The check digit (last digit) that WOULD make the number valid — handy for
    /// generating a valid number or spotting a single-digit typo.
    pub expected_check_digit: u8,
    /// Best-effort payment-card brand when the length/prefix matches.
    pub brand: Option<String>,
}

/// Clean input to digits only; error on unexpected characters.
fn clean(input: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for c in input.chars() {
        match c {
            '0'..='9' => out.push(c as u8 - b'0'),
            ' ' | '-' | '\t' | '_' => {}
            other => return Err(format!("unexpected character '{other}' (only digits, spaces, and dashes allowed)")),
        }
    }
    if out.is_empty() {
        return Err("no digits found".into());
    }
    Ok(out)
}

/// Luhn sum over `digits` treating the rightmost as position 0 (doubled positions
/// are the odd indices from the right).
fn luhn_sum(digits: &[u8]) -> u32 {
    let mut sum = 0u32;
    for (i, &d) in digits.iter().rev().enumerate() {
        let mut v = d as u32;
        if i % 2 == 1 {
            v *= 2;
            if v > 9 {
                v -= 9;
            }
        }
        sum += v;
    }
    sum
}

fn detect_brand(d: &[u8]) -> Option<String> {
    let n = d.len();
    let s: String = d.iter().map(|x| (x + b'0') as char).collect();
    let p = |k: usize| -> u32 { s[..k.min(s.len())].parse().unwrap_or(0) };
    let two = p(2);
    let four = p(4);
    let brand = if d.first() == Some(&4) && (n == 13 || n == 16 || n == 19) {
        "Visa"
    } else if (34 == two || 37 == two) && n == 15 {
        "American Express"
    } else if ((51..=55).contains(&two) || (2221..=2720).contains(&four)) && n == 16 {
        "Mastercard"
    } else if (four == 6011 || two == 65 || (644..=649).contains(&p(3))) && n == 16 {
        "Discover"
    } else if (3528..=3589).contains(&four) && n == 16 {
        "JCB"
    } else if (two == 36 || two == 38 || (300..=305).contains(&p(3))) && (n == 14 || n == 16) {
        "Diners Club"
    } else {
        return None;
    };
    Some(brand.to_string())
}

/// Validate `input` with the Luhn algorithm.
pub fn check(input: &str) -> Result<LuhnResult, String> {
    let digits = clean(input)?;
    if digits.len() < 2 {
        return Err("need at least 2 digits to run a Luhn check".into());
    }
    let valid = luhn_sum(&digits) % 10 == 0;

    // Check digit that would make the whole number valid: zero the last position,
    // sum, then pick the digit completing it to a multiple of 10.
    let mut probe = digits.clone();
    *probe.last_mut().unwrap() = 0;
    let s = luhn_sum(&probe);
    let expected_check_digit = ((10 - (s % 10)) % 10) as u8;

    let digit_str: String = digits.iter().map(|x| (x + b'0') as char).collect();
    let brand = detect_brand(&digits);

    Ok(LuhnResult {
        valid,
        length: digits.len(),
        expected_check_digit,
        brand,
        digits: digit_str,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_visa_test_card() {
        let r = check("4242 4242 4242 4242").unwrap();
        assert!(r.valid);
        assert_eq!(r.digits, "4242424242424242");
        assert_eq!(r.length, 16);
        assert_eq!(r.brand.as_deref(), Some("Visa"));
    }

    #[test]
    fn invalid_card_off_by_one() {
        let r = check("4242424242424241").unwrap();
        assert!(!r.valid);
        // the correct last digit is 2
        assert_eq!(r.expected_check_digit, 2);
    }

    #[test]
    fn valid_imei() {
        // 490154203237518 is a well-known Luhn-valid IMEI.
        let r = check("490154203237518").unwrap();
        assert!(r.valid);
        assert_eq!(r.length, 15);
        assert_eq!(r.brand, None); // not a card length/prefix
    }

    #[test]
    fn amex_and_mastercard_brands() {
        assert_eq!(check("378282246310005").unwrap().brand.as_deref(), Some("American Express"));
        assert_eq!(check("5555555555554444").unwrap().brand.as_deref(), Some("Mastercard"));
    }

    #[test]
    fn dashes_and_spaces_ignored() {
        assert_eq!(check("4242-4242-4242-4242").unwrap().digits, "4242424242424242");
    }

    #[test]
    fn errors() {
        assert!(check("").is_err());
        assert!(check("12ab").is_err());
        assert!(check("7").is_err()); // too short
    }
}
