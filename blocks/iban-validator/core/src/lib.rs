//! gizza-ai/iban-validator core — validate an IBAN (International Bank Account
//! Number) per ISO 13616: country-aware length check + the mod-97 checksum
//! (ISO 7064), then parse out the country, check digits and BBAN (plus the bank
//! identifier when the country's structure makes it unambiguous). No deps; pure.
//! Spaces are ignored and input is upper-cased before checking.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IbanResult {
    /// True if the IBAN is well-formed, has the correct length for its country,
    /// and passes the mod-97 checksum.
    pub valid: bool,
    /// The normalized IBAN (spaces removed, upper-cased).
    pub normalized: String,
    /// The IBAN grouped in 4-character blocks for display, e.g. "GB82 WEST …".
    pub formatted: String,
    /// ISO 3166-1 alpha-2 country code (first two characters).
    pub country_code: String,
    /// Full country name when known.
    pub country: Option<String>,
    /// The two ISO 7064 check digits (characters 3-4).
    pub check_digits: String,
    /// Basic Bank Account Number — everything after the check digits.
    pub bban: String,
    /// Total character length.
    pub length: usize,
    /// Bank identifier extracted from the BBAN when the country structure is known.
    pub bank_code: Option<String>,
    /// Branch (sort) code extracted from the BBAN when the country has one.
    pub branch_code: Option<String>,
    /// Account number extracted from the BBAN when the country structure is known.
    pub account_number: Option<String>,
}

/// Country -> (expected total IBAN length, full name).
/// Lengths from the SWIFT IBAN registry.
const REGISTRY: &[(&str, usize, &str)] = &[
    ("AD", 24, "Andorra"),
    ("AE", 23, "United Arab Emirates"),
    ("AL", 28, "Albania"),
    ("AT", 20, "Austria"),
    ("AZ", 28, "Azerbaijan"),
    ("BA", 20, "Bosnia and Herzegovina"),
    ("BE", 16, "Belgium"),
    ("BG", 22, "Bulgaria"),
    ("BH", 22, "Bahrain"),
    ("BR", 29, "Brazil"),
    ("BY", 28, "Belarus"),
    ("CH", 21, "Switzerland"),
    ("CR", 22, "Costa Rica"),
    ("CY", 28, "Cyprus"),
    ("CZ", 24, "Czech Republic"),
    ("DE", 22, "Germany"),
    ("DK", 18, "Denmark"),
    ("DO", 28, "Dominican Republic"),
    ("EE", 20, "Estonia"),
    ("EG", 29, "Egypt"),
    ("ES", 24, "Spain"),
    ("FI", 18, "Finland"),
    ("FO", 18, "Faroe Islands"),
    ("FR", 27, "France"),
    ("GB", 22, "United Kingdom"),
    ("GE", 22, "Georgia"),
    ("GI", 23, "Gibraltar"),
    ("GL", 18, "Greenland"),
    ("GR", 27, "Greece"),
    ("GT", 28, "Guatemala"),
    ("HR", 21, "Croatia"),
    ("HU", 28, "Hungary"),
    ("IE", 22, "Ireland"),
    ("IL", 23, "Israel"),
    ("IQ", 23, "Iraq"),
    ("IS", 26, "Iceland"),
    ("IT", 27, "Italy"),
    ("JO", 30, "Jordan"),
    ("KW", 30, "Kuwait"),
    ("KZ", 20, "Kazakhstan"),
    ("LB", 28, "Lebanon"),
    ("LC", 32, "Saint Lucia"),
    ("LI", 21, "Liechtenstein"),
    ("LT", 20, "Lithuania"),
    ("LU", 20, "Luxembourg"),
    ("LV", 21, "Latvia"),
    ("LY", 25, "Libya"),
    ("MC", 27, "Monaco"),
    ("MD", 24, "Moldova"),
    ("ME", 22, "Montenegro"),
    ("MK", 19, "North Macedonia"),
    ("MR", 27, "Mauritania"),
    ("MT", 31, "Malta"),
    ("MU", 30, "Mauritius"),
    ("NL", 18, "Netherlands"),
    ("NO", 15, "Norway"),
    ("PK", 24, "Pakistan"),
    ("PL", 28, "Poland"),
    ("PS", 29, "Palestine"),
    ("PT", 25, "Portugal"),
    ("QA", 29, "Qatar"),
    ("RO", 24, "Romania"),
    ("RS", 22, "Serbia"),
    ("SA", 24, "Saudi Arabia"),
    ("SC", 31, "Seychelles"),
    ("SE", 24, "Sweden"),
    ("SI", 19, "Slovenia"),
    ("SK", 24, "Slovakia"),
    ("SM", 27, "San Marino"),
    ("ST", 25, "Sao Tome and Principe"),
    ("SV", 28, "El Salvador"),
    ("TL", 23, "Timor-Leste"),
    ("TN", 24, "Tunisia"),
    ("TR", 26, "Turkey"),
    ("UA", 29, "Ukraine"),
    ("VA", 22, "Vatican City"),
    ("VG", 24, "Virgin Islands, British"),
    ("XK", 20, "Kosovo"),
];

fn lookup(cc: &str) -> Option<(usize, &'static str)> {
    REGISTRY
        .iter()
        .find(|(c, _, _)| *c == cc)
        .map(|(_, len, name)| (*len, *name))
}

/// Bank / branch / account split for common countries with a well-known BBAN
/// structure. Returns (bank_code, branch_code, account_number).
fn split_bban(cc: &str, bban: &str) -> (Option<String>, Option<String>, Option<String>) {
    let take = |a: usize, b: usize| -> Option<String> { bban.get(a..b).map(|s| s.to_string()) };
    match cc {
        // GB/IE: 4 bank (BIC) + 6 sort/branch + 8 account
        "GB" | "IE" => (take(0, 4), take(4, 10), take(10, 18)),
        // DE: 8 bank + 10 account (no branch)
        "DE" => (take(0, 8), None, take(8, 18)),
        // FR/MC: 5 bank + 5 branch + 11 account + 2 key
        "FR" | "MC" => (take(0, 5), take(5, 10), take(10, 21)),
        // ES: 4 bank + 4 branch + 2 check + 10 account
        "ES" => (take(0, 4), take(4, 8), take(10, 20)),
        // NL: 4 bank (alpha) + 10 account (no branch)
        "NL" => (take(0, 4), None, take(4, 14)),
        // IT/SM: 1 CIN + 5 ABI(bank) + 5 CAB(branch) + 12 account
        "IT" | "SM" => (take(1, 6), take(6, 11), take(11, 23)),
        // BE: 3 bank + 7 account + 2 check (no branch)
        "BE" => (take(0, 3), None, take(3, 10)),
        // CH/LI: 5 bank + 12 account (no branch)
        "CH" | "LI" => (take(0, 5), None, take(5, 17)),
        // AT: 5 bank + 11 account (no branch)
        "AT" => (take(0, 5), None, take(5, 16)),
        _ => (None, None, None),
    }
}

/// Normalize: remove whitespace and upper-case.
fn normalize(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// Group into 4-character blocks for display.
fn group4(s: &str) -> String {
    s.as_bytes()
        .chunks(4)
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ")
}

/// ISO 7064 mod-97-10 over the rearranged string: each letter maps to two digits
/// (A=10 … Z=35); the running remainder is folded to stay in u32 range.
fn mod97(s: &str) -> u32 {
    let mut rem: u32 = 0;
    for c in s.chars() {
        if c.is_ascii_digit() {
            rem = (rem * 10 + (c as u32 - '0' as u32)) % 97;
        } else {
            let v = c as u32 - 'A' as u32 + 10;
            rem = (rem * 100 + v) % 97;
        }
    }
    rem
}

/// Validate `input` as an IBAN.
pub fn validate(input: &str) -> Result<IbanResult, String> {
    let normalized = normalize(input);
    if normalized.is_empty() {
        return Err("no IBAN provided".into());
    }
    if normalized.len() < 5 {
        return Err("IBAN is too short".into());
    }
    let bytes = normalized.as_bytes();
    if !bytes[0].is_ascii_alphabetic() || !bytes[1].is_ascii_alphabetic() {
        return Err("IBAN must start with a two-letter country code".into());
    }
    if !bytes[2].is_ascii_digit() || !bytes[3].is_ascii_digit() {
        return Err("characters 3-4 of an IBAN must be the two check digits".into());
    }
    if !normalized.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("IBAN may only contain letters and digits".into());
    }

    let country_code = normalized[..2].to_string();
    let check_digits = normalized[2..4].to_string();
    let bban = normalized[4..].to_string();
    let length = normalized.len();

    let (expected_len, country) = match lookup(&country_code) {
        Some((l, n)) => (Some(l), Some(n.to_string())),
        None => (None, None),
    };

    let length_ok = match expected_len {
        Some(l) => length == l,
        None => true, // unknown country: can't length-check, still checksum it
    };

    // Rearrange: move the first four chars to the end, then mod-97 must equal 1.
    let rearranged = format!("{}{}", &normalized[4..], &normalized[..4]);
    let checksum_ok = mod97(&rearranged) == 1;

    let valid = length_ok && checksum_ok;

    let (bank_code, branch_code, account_number) = if valid {
        split_bban(&country_code, &bban)
    } else {
        (None, None, None)
    };

    Ok(IbanResult {
        valid,
        normalized: normalized.clone(),
        formatted: group4(&normalized),
        country_code,
        country,
        check_digits,
        bban,
        length,
        bank_code,
        branch_code,
        account_number,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_uk_iban() {
        let r = validate("GB82 WEST 1234 5698 7654 32").unwrap();
        assert!(r.valid);
        assert_eq!(r.country_code, "GB");
        assert_eq!(r.country.as_deref(), Some("United Kingdom"));
        assert_eq!(r.check_digits, "82");
        assert_eq!(r.length, 22);
        assert_eq!(r.formatted, "GB82 WEST 1234 5698 7654 32");
        assert_eq!(r.bank_code.as_deref(), Some("WEST"));
        assert_eq!(r.branch_code.as_deref(), Some("123456"));
        assert_eq!(r.account_number.as_deref(), Some("98765432"));
    }

    #[test]
    fn valid_german_iban() {
        let r = validate("DE89370400440532013000").unwrap();
        assert!(r.valid);
        assert_eq!(r.country.as_deref(), Some("Germany"));
        assert_eq!(r.bank_code.as_deref(), Some("37040044"));
        assert_eq!(r.account_number.as_deref(), Some("0532013000"));
    }

    #[test]
    fn valid_french_and_dutch() {
        assert!(validate("FR1420041010050500013M02606").unwrap().valid);
        assert!(validate("NL91ABNA0417164300").unwrap().valid);
    }

    #[test]
    fn bad_check_digits() {
        let r = validate("GB83WEST12345698765432").unwrap();
        assert!(!r.valid);
    }

    #[test]
    fn wrong_length_for_country() {
        let r = validate("GB82WEST123456987654").unwrap();
        assert!(!r.valid);
    }

    #[test]
    fn lowercase_and_spaces_normalized() {
        let r = validate("de89 3704 0044 0532 0130 00").unwrap();
        assert!(r.valid);
        assert_eq!(r.normalized, "DE89370400440532013000");
    }

    #[test]
    fn unknown_country_still_runs() {
        let r = validate("ZZ00ABCD1234").unwrap();
        assert!(r.country.is_none());
        assert_eq!(r.country_code, "ZZ");
    }

    #[test]
    fn errors() {
        assert!(validate("").is_err());
        assert!(validate("X").is_err());
        assert!(validate("1234567890").is_err()); // no country letters
        assert!(validate("GBXXWEST12345698765432").is_err()); // check digits not numeric
        assert!(validate("GB82WEST1234569876543$").is_err()); // bad char
    }
}
