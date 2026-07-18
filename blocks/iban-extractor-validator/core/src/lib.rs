//! gizza-ai/iban-extractor-validator core — find every IBAN inside free-form
//! text (invoices, emails, statements, logs) and validate each with the ISO
//! 13616 mod-97 checksum + the country-specific length.
//!
//! Candidates are anchored at an IBAN start (`\b` + two letters + two check
//! digits) whose country code is in the SWIFT registry. From each anchor we
//! read EXACTLY the country's expected number of alphanumeric characters,
//! tolerating the usual 4-character grouping spaces, then hand the normalized
//! string to `iban-validator`'s `validate()` (single source of truth for the
//! checksum + BBAN parsing). Valid and invalid (structurally IBAN-shaped but
//! failing the checksum) matches are returned separately, deduplicated, in
//! first-seen order. Pure-Rust → runs on every backend.

use gizza_ai_iban_validator_core::{expected_length, validate};
use regex::Regex;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidIban {
    /// The IBAN grouped in 4-character blocks for display, e.g. "GB82 WEST …".
    pub formatted: String,
    /// The normalized IBAN (spaces removed, upper-cased).
    pub normalized: String,
    /// ISO 3166-1 alpha-2 country code.
    pub country_code: String,
    /// Full country name when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    /// Bank identifier parsed from the BBAN when the country structure is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_code: Option<String>,
    /// Account number parsed from the BBAN when the country structure is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_number: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InvalidIban {
    /// The candidate grouped in 4-character blocks for display.
    pub formatted: String,
    /// The normalized candidate (spaces removed, upper-cased).
    pub normalized: String,
    /// ISO 3166-1 alpha-2 country code.
    pub country_code: String,
    /// Why the candidate failed (e.g. "failed the mod-97 checksum").
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Found {
    /// Total distinct IBAN candidates found (valid + invalid).
    pub count: usize,
    pub valid_count: usize,
    pub invalid_count: usize,
    /// Valid IBANs, first-seen order, deduplicated.
    pub valid: Vec<ValidIban>,
    /// Structurally IBAN-shaped candidates that failed validation.
    pub invalid: Vec<InvalidIban>,
}

thread_local! {
    // Anchor: a word boundary, two letters (country), two digits (check digits).
    // Case-insensitive; the country/length/checksum are verified afterwards.
    static ANCHOR: Regex = Regex::new(r"(?i)\b[A-Z]{2}[0-9]{2}").unwrap();
}

/// From byte offset `start` in `text`, read exactly `want` alphanumeric ASCII
/// characters, allowing a single ASCII space or tab between them (the standard
/// 4-character grouping). Returns the normalized (upper-cased, space-stripped)
/// string, or `None` if fewer than `want` alphanumerics are available before the
/// run ends, or if the IBAN is glued to a longer alphanumeric token.
fn read_candidate(text: &str, start: usize, want: usize) -> Option<String> {
    let mut out = String::with_capacity(want);
    let mut chars = text[start..].chars().peekable();
    while out.len() < want {
        match chars.next() {
            Some(c) if c.is_ascii_alphanumeric() => out.push(c.to_ascii_uppercase()),
            // Tolerate a single grouping space, but only if an alphanumeric
            // follows it — a trailing/double space ends the token.
            Some(c) if c == ' ' || c == '\t' => match chars.peek() {
                Some(n) if n.is_ascii_alphanumeric() => continue,
                _ => return None,
            },
            _ => return None,
        }
    }
    // The character right after the collected IBAN. If it is alphanumeric the
    // IBAN is glued to a longer token (e.g. an over-long account blob) and is
    // not a clean standalone match.
    if matches!(chars.next(), Some(c) if c.is_ascii_alphanumeric()) {
        return None;
    }
    Some(out)
}

/// Find, validate and deduplicate every IBAN in `text`.
pub fn extract(text: &str) -> Found {
    let mut valid: Vec<ValidIban> = Vec::new();
    let mut invalid: Vec<InvalidIban> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    ANCHOR.with(|re| {
        for m in re.find_iter(text) {
            let anchor = m.as_str();
            // country code = first two letters, upper-cased.
            let cc: String = anchor[..2].to_ascii_uppercase();
            let want = match expected_length(&cc) {
                Some(l) => l,
                None => continue, // country has no IBAN — skip.
            };
            let normalized = match read_candidate(text, m.start(), want) {
                Some(v) => v,
                None => continue,
            };
            if !seen.insert(normalized.clone()) {
                continue; // already reported.
            }
            match validate(&normalized) {
                Ok(r) if r.valid => valid.push(ValidIban {
                    formatted: r.formatted,
                    normalized: r.normalized,
                    country_code: r.country_code,
                    country: r.country,
                    bank_code: r.bank_code,
                    account_number: r.account_number,
                }),
                Ok(r) => invalid.push(InvalidIban {
                    formatted: r.formatted,
                    normalized: r.normalized,
                    country_code: r.country_code,
                    reason: "failed the mod-97 checksum".to_string(),
                }),
                // A candidate we already length-matched for a known country
                // should parse structurally; treat any hard error defensively.
                Err(e) => invalid.push(InvalidIban {
                    formatted: normalized.clone(),
                    normalized,
                    country_code: cc,
                    reason: e,
                }),
            }
        }
    });

    Found {
        count: valid.len() + invalid.len(),
        valid_count: valid.len(),
        invalid_count: invalid.len(),
        valid,
        invalid,
    }
}

/// Human-readable rendering used by the page.
pub fn render(text: &str) -> Result<String, String> {
    let r = extract(text);
    if r.count == 0 {
        return Ok("No IBANs found.".to_string());
    }
    let mut out = format!(
        "Found {} IBAN(s): {} valid, {} invalid.\n",
        r.count, r.valid_count, r.invalid_count
    );
    if !r.valid.is_empty() {
        out.push_str(&format!("\nValid ({}):\n", r.valid_count));
        for v in &r.valid {
            let place = v.country.as_deref().unwrap_or(&v.country_code);
            out.push_str(&format!("  {}  -  {}\n", v.formatted, place));
        }
    }
    if !r.invalid.is_empty() {
        out.push_str(&format!("\nInvalid ({}):\n", r.invalid_count));
        for i in &r.invalid {
            out.push_str(&format!("  {}  -  {}\n", i.formatted, i.reason));
        }
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_valid_iban_in_prose() {
        let t = "Please wire to GB82 WEST 1234 5698 7654 32 by Friday.";
        let r = extract(t);
        assert_eq!(r.valid_count, 1);
        assert_eq!(r.invalid_count, 0);
        assert_eq!(r.valid[0].normalized, "GB82WEST12345698765432");
        assert_eq!(r.valid[0].country.as_deref(), Some("United Kingdom"));
        assert_eq!(r.valid[0].formatted, "GB82 WEST 1234 5698 7654 32");
    }

    #[test]
    fn contiguous_and_multiple() {
        // A German IBAN written without spaces, plus a French one, in one blob.
        let t = "DE89370400440532013000 / FR14 2004 1010 0505 0001 3M02 606";
        let r = extract(t);
        assert_eq!(r.valid_count, 2);
        assert!(r.valid.iter().any(|v| v.country_code == "DE"));
        assert!(r.valid.iter().any(|v| v.country_code == "FR"));
    }

    #[test]
    fn dedupes_repeats() {
        let t = "GB82WEST12345698765432 and again GB82 WEST 1234 5698 7654 32";
        let r = extract(t);
        assert_eq!(r.valid_count, 1);
    }

    #[test]
    fn flags_checksum_typo_as_invalid() {
        // Same UK IBAN with the last digit changed — right length, bad checksum.
        let t = "Old account was GB82 WEST 1234 5698 7654 31 (now closed).";
        let r = extract(t);
        assert_eq!(r.valid_count, 0);
        assert_eq!(r.invalid_count, 1);
        assert_eq!(r.invalid[0].country_code, "GB");
        assert!(r.invalid[0].reason.contains("mod-97"));
    }

    #[test]
    fn ignores_non_iban_country_and_short_runs() {
        // "US" has no IBAN; "GB12" is too short to be a UK IBAN.
        let t = "Account US12 3456 7890 and code GB12 only.";
        let r = extract(t);
        assert_eq!(r.count, 0);
    }

    #[test]
    fn no_ibans() {
        assert_eq!(extract("just some ordinary text, no bank details").count, 0);
    }

    #[test]
    fn render_reports_valid_and_invalid() {
        let out =
            render("Pay GB82 WEST 1234 5698 7654 32 not GB82 WEST 1234 5698 7654 31").unwrap();
        assert!(out.contains("1 valid, 1 invalid"));
        assert!(out.contains("United Kingdom"));
        assert!(out.contains("mod-97"));
    }

    #[test]
    fn render_empty() {
        assert_eq!(render("nothing here").unwrap(), "No IBANs found.");
    }
}
