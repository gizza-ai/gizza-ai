//! gizza-ai/luhn-checkdigit core — compute the Luhn (mod-10) check digit for a
//! PARTIAL number (the payload WITHOUT its check digit) and return the completed,
//! valid number. This is the generator counterpart to luhn-validate: there the
//! input's last digit IS the check digit (and we validate it); here every input
//! digit is payload and we append the missing check digit. No deps; pure.
//! Spaces and dashes in the input are ignored.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckDigitResult {
    /// The Luhn check digit (0-9) that makes the whole number valid.
    pub check_digit: u8,
    /// The cleaned payload digits (spaces/dashes removed), WITHOUT the check digit.
    pub payload: String,
    /// The full number: payload with the check digit appended.
    pub full_number: String,
    /// Length of the full number (payload + 1).
    pub length: usize,
}

/// Clean input to digits only; error on unexpected characters.
fn clean(input: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for c in input.chars() {
        match c {
            '0'..='9' => out.push(c as u8 - b'0'),
            ' ' | '-' | '\t' | '_' => {}
            other => {
                return Err(format!(
                    "unexpected character '{other}' (only digits, spaces, and dashes allowed)"
                ))
            }
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

/// Compute the Luhn check digit for `input`, treating every digit as payload.
pub fn check_digit(input: &str) -> Result<CheckDigitResult, String> {
    let payload = clean(input)?;
    // The check digit sits at position 0 of the FULL number, so the payload digits
    // shift up by one position. Append a placeholder 0 and compute the sum: the
    // check digit is what completes that sum to a multiple of 10.
    let mut probe = payload.clone();
    probe.push(0);
    let s = luhn_sum(&probe);
    let check_digit = ((10 - (s % 10)) % 10) as u8;

    let payload_str: String = payload.iter().map(|x| (x + b'0') as char).collect();
    let full_number = format!("{payload_str}{check_digit}");
    let length = full_number.len();

    Ok(CheckDigitResult {
        check_digit,
        payload: payload_str,
        full_number,
        length,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_wikipedia_example() {
        // Wikipedia's canonical example: payload 7992739871 → check digit 3.
        let r = check_digit("7992739871").unwrap();
        assert_eq!(r.check_digit, 3);
        assert_eq!(r.payload, "7992739871");
        assert_eq!(r.full_number, "79927398713");
        assert_eq!(r.length, 11);
    }

    #[test]
    fn completes_visa_test_card_prefix() {
        // First 15 digits of the 4242... test card → check digit 2 → full 4242...4242.
        let r = check_digit("424242424242424").unwrap();
        assert_eq!(r.check_digit, 2);
        assert_eq!(r.full_number, "4242424242424242");
    }

    #[test]
    fn ignores_spaces_and_dashes() {
        let r = check_digit("4242-4242 4242 424").unwrap();
        assert_eq!(r.payload, "424242424242424");
        assert_eq!(r.check_digit, 2);
    }

    #[test]
    fn single_digit_payload_ok() {
        // Payload "0" → sum with appended 0 doubles the leading 0 → check digit 0.
        let r = check_digit("0").unwrap();
        assert_eq!(r.check_digit, 0);
        assert_eq!(r.full_number, "00");
    }

    #[test]
    fn output_passes_luhn() {
        // Sanity: the produced full number always validates (sum % 10 == 0).
        for s in ["1", "12345", "7992739871", "100000000000"] {
            let r = check_digit(s).unwrap();
            let digits: Vec<u8> = r.full_number.bytes().map(|b| b - b'0').collect();
            assert_eq!(
                luhn_sum(&digits) % 10,
                0,
                "full number {} must be valid",
                r.full_number
            );
        }
    }

    #[test]
    fn empty_input_errors() {
        assert!(check_digit("   ").is_err());
        assert!(check_digit("").is_err());
    }

    #[test]
    fn bad_char_errors() {
        let e = check_digit("12a45").unwrap_err();
        assert!(e.contains("unexpected character"));
    }
}
