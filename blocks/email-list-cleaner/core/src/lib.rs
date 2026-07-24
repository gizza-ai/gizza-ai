//! email-list-cleaner core — clean a pasted list of email addresses.
//!
//! Pure compute, no wafer/wasm-bindgen deps. Shared by the chat skill block and
//! the web page. Takes a free-form list (one address per line, or comma-/
//! semicolon-separated) and, for every non-blank entry:
//!   1. validates its syntax (RFC 5321/5322 subset — reuses `email-validator`);
//!   2. trims wrappers (`mailto:`, `Name <addr>`) and lowercases it (reuses
//!      `email-normalizer`); optionally applies Gmail-style canonicalization
//!      (drop dots + `+tag`) so aliases collapse to one address;
//!   3. de-duplicates on the cleaned form, preserving first-seen order (or
//!      alphabetical when asked);
//!   4. reports malformed entries with the reason, and surfaces likely typos
//!      (e.g. `gmial.com` -> `gmail.com`) as suggestions.
//!
//! It is a *syntax* cleaner — it never touches the network, so it does not do
//! MX/DNS or SMTP mailbox verification, nor disposable-domain detection.

use std::collections::HashSet;

use gizza_ai_email_normalizer_core::normalize;
use gizza_ai_email_validator_core::validate;

/// Output format for [`report`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Full breakdown: counts summary, valid list, possible typos, invalid list.
    Report,
    /// Only the cleaned, unique, valid addresses — one per line (copy-ready).
    Clean,
    /// The cleaned, unique, valid addresses joined by `, ` (paste into a To: field).
    Comma,
}

impl Format {
    /// Parse a format name (case-insensitive). `report` is the default fallback.
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "report" => Ok(Format::Report),
            "clean" | "list" => Ok(Format::Clean),
            "comma" => Ok(Format::Comma),
            other => Err(format!(
                "invalid format {other:?}: expected 'report', 'clean', or 'comma'"
            )),
        }
    }
}

/// A row that failed validation, kept for the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidRow {
    /// The entry exactly as it appeared in the input (trimmed of outer space).
    pub input: String,
    /// A human-readable reason it was rejected.
    pub reason: String,
}

/// A likely-typo entry: a syntactically valid address whose domain/TLD looks
/// misspelled, with the tool's best-guess correction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypoRow {
    /// The cleaned address as it was accepted.
    pub address: String,
    /// The suggested corrected address.
    pub suggestion: String,
}

/// The full result of cleaning a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanReport {
    /// Number of non-blank entries seen in the input.
    pub total: usize,
    /// The cleaned, unique, valid addresses (in the requested order).
    pub valid: Vec<String>,
    /// Number of valid entries dropped because they duplicated an earlier one.
    pub duplicates: usize,
    /// Entries that failed validation.
    pub invalid: Vec<InvalidRow>,
    /// Likely typos among the valid addresses (deduped, first-seen order).
    pub typos: Vec<TypoRow>,
}

impl CleanReport {
    /// Count of unique valid addresses kept.
    pub fn valid_count(&self) -> usize {
        self.valid.len()
    }
    /// Count of invalid entries.
    pub fn invalid_count(&self) -> usize {
        self.invalid.len()
    }
}

/// Split the raw input into candidate entries on newlines, commas, and
/// semicolons. Whitespace is NOT a separator so `Name <addr>` display-name rows
/// survive intact; each entry is returned trimmed, and blank entries are dropped.
fn split_entries(input: &str) -> Vec<String> {
    input
        .split(|c| c == '\n' || c == '\r' || c == ',' || c == ';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Clean the list. `canonicalize` folds provider aliases (Gmail dots + `+tag`)
/// into their canonical delivery form before de-duplicating; `sort_alpha` sorts
/// the surviving addresses alphabetically instead of keeping first-seen order.
pub fn clean(input: &str, canonicalize: bool, sort_alpha: bool) -> CleanReport {
    let mut valid: Vec<String> = Vec::new();
    let mut invalid: Vec<InvalidRow> = Vec::new();
    let mut typos: Vec<TypoRow> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut seen_typo: HashSet<String> = HashSet::new();
    let mut duplicates = 0usize;
    let mut total = 0usize;

    for entry in split_entries(input) {
        total += 1;
        let v = validate(&entry);
        if !v.valid {
            let reason = if v.errors.is_empty() {
                "invalid address".to_string()
            } else {
                v.errors.join("; ")
            };
            invalid.push(InvalidRow {
                input: entry,
                reason,
            });
            continue;
        }

        // Valid -> derive the cleaned form. `normalize` gives us trim + lowercase
        // (`cleaned`) and the provider-canonical form (`canonical`). If it can't
        // parse an address the validator accepted (rare edge, e.g. a quoted
        // local part), fall back to the validator's stripped, lowercased form.
        let cleaned_addr = match normalize(&entry, true) {
            Ok(n) => {
                if canonicalize {
                    n.canonical
                } else {
                    n.cleaned
                }
            }
            Err(_) => v.normalized.to_ascii_lowercase(),
        };

        if seen.insert(cleaned_addr.clone()) {
            // Surface a typo suggestion for this address, if the validator has one.
            if let Some(sugg) = &v.suggestion {
                let sugg_clean = normalize(sugg, true)
                    .map(|n| if canonicalize { n.canonical } else { n.cleaned })
                    .unwrap_or_else(|_| sugg.to_ascii_lowercase());
                if sugg_clean != cleaned_addr && seen_typo.insert(cleaned_addr.clone()) {
                    typos.push(TypoRow {
                        address: cleaned_addr.clone(),
                        suggestion: sugg_clean,
                    });
                }
            }
            valid.push(cleaned_addr);
        } else {
            duplicates += 1;
        }
    }

    if sort_alpha {
        valid.sort();
    }

    CleanReport {
        total,
        valid,
        duplicates,
        invalid,
        typos,
    }
}

/// Clean the list and render the chosen `format` as a string. This is the shared
/// text surface used by both the chat skill and the standalone page so the
/// output is identical everywhere.
pub fn report(
    input: &str,
    canonicalize: bool,
    sort_alpha: bool,
    format: &str,
) -> Result<String, String> {
    let fmt = Format::parse(format)?;
    let r = clean(input, canonicalize, sort_alpha);

    match fmt {
        Format::Clean => Ok(r.valid.join("\n")),
        Format::Comma => Ok(r.valid.join(", ")),
        Format::Report => {
            let mut out = String::new();
            out.push_str("Summary\n");
            out.push_str(&format!("  Entries processed: {}\n", r.total));
            out.push_str(&format!("  Valid unique: {}\n", r.valid_count()));
            out.push_str(&format!("  Duplicates removed: {}\n", r.duplicates));
            out.push_str(&format!("  Invalid: {}\n", r.invalid_count()));

            out.push_str(&format!("\nValid ({}):\n", r.valid.len()));
            if r.valid.is_empty() {
                out.push_str("  (none)\n");
            } else {
                for a in &r.valid {
                    out.push_str(a);
                    out.push('\n');
                }
            }

            if !r.typos.is_empty() {
                out.push_str(&format!("\nPossible typos ({}):\n", r.typos.len()));
                for t in &r.typos {
                    out.push_str(&format!("  {} -> {}\n", t.address, t.suggestion));
                }
            }

            if !r.invalid.is_empty() {
                out.push_str(&format!("\nInvalid ({}):\n", r.invalid.len()));
                for row in &r.invalid {
                    out.push_str(&format!("  {} — {}\n", row.input, row.reason));
                }
            }

            Ok(out.trim_end().to_string())
        }
    }
}

/// Browser/page compatibility entry point: clean using defaults (no provider
/// alias canonicalization, input order, full report).
pub fn run(input: &str) -> Result<String, String> {
    report(input, false, false, "report")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_dedupe_and_normalize() {
        // Mixed case + duplicates + whitespace + a mailto/wrapper form.
        let input =
            "Alice@Example.com\n  BOB@example.com  \nalice@example.com\nmailto:carol@Example.COM";
        let r = clean(input, false, false);
        assert_eq!(r.total, 4);
        assert_eq!(
            r.valid,
            vec![
                "alice@example.com".to_string(),
                "bob@example.com".to_string(),
                "carol@example.com".to_string(),
            ]
        );
        assert_eq!(r.duplicates, 1); // the repeated alice
        assert!(r.invalid.is_empty());
    }

    #[test]
    fn flags_malformed_entries() {
        let input = "good@example.com, not-an-email, missing@dot, @nodomain.com";
        let r = clean(input, false, false);
        assert_eq!(r.valid, vec!["good@example.com".to_string()]);
        assert_eq!(r.invalid.len(), 3);
        assert_eq!(r.invalid[0].input, "not-an-email");
        assert!(!r.invalid[0].reason.is_empty());
    }

    #[test]
    fn comma_and_semicolon_delimited() {
        let input = "a@x.com; b@x.com, c@x.com";
        let r = clean(input, false, false);
        assert_eq!(r.valid.len(), 3);
        assert_eq!(r.total, 3);
    }

    #[test]
    fn canonicalize_folds_gmail_aliases() {
        // Without canonicalization these are three distinct cleaned forms; with
        // it, Gmail dot/+tag folding collapses them to one.
        let input = "john.doe+news@gmail.com\njohndoe@gmail.com\nJohnDoe@googlemail.com";
        let plain = clean(input, false, false);
        assert_eq!(plain.valid.len(), 3);
        let canon = clean(input, true, false);
        assert_eq!(canon.valid, vec!["johndoe@gmail.com".to_string()]);
        assert_eq!(canon.duplicates, 2);
    }

    #[test]
    fn sort_alpha_orders_output() {
        let input = "zeta@x.com\nalpha@x.com\nmid@x.com";
        let r = clean(input, false, true);
        assert_eq!(
            r.valid,
            vec![
                "alpha@x.com".to_string(),
                "mid@x.com".to_string(),
                "zeta@x.com".to_string(),
            ]
        );
    }

    #[test]
    fn surfaces_typo_suggestion() {
        let r = clean("user@gmial.com", false, false);
        assert_eq!(r.valid, vec!["user@gmial.com".to_string()]);
        assert_eq!(r.typos.len(), 1);
        assert_eq!(r.typos[0].suggestion, "user@gmail.com");
    }

    #[test]
    fn report_format_clean_is_just_the_list() {
        let out = report("b@x.com\na@x.com\nb@x.com", false, true, "clean").unwrap();
        assert_eq!(out, "a@x.com\nb@x.com");
    }

    #[test]
    fn report_format_comma_joins() {
        let out = report("a@x.com, b@x.com", false, false, "comma").unwrap();
        assert_eq!(out, "a@x.com, b@x.com");
    }

    #[test]
    fn report_full_has_summary_and_sections() {
        let out = report("good@x.com\nbad@\ngood@x.com", false, false, "report").unwrap();
        assert!(out.contains("Entries processed: 3"));
        assert!(out.contains("Valid unique: 1"));
        assert!(out.contains("Duplicates removed: 1"));
        assert!(out.contains("Invalid: 1"));
        assert!(out.contains("good@x.com"));
        assert!(out.contains("Invalid (1):"));
    }

    #[test]
    fn bad_format_errors() {
        let err = report("a@x.com", false, false, "xml").unwrap_err();
        assert!(err.contains("invalid format"));
    }

    #[test]
    fn empty_input_is_all_zero() {
        let r = clean("   \n , ; \n  ", false, false);
        assert_eq!(r.total, 0);
        assert!(r.valid.is_empty());
        assert!(r.invalid.is_empty());
    }
}
