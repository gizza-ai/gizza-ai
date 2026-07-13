//! gizza-ai/contact-info-extractor core — pull email addresses and phone numbers
//! out of unstructured text, deduplicate, count, and optionally sort. Pure-Rust
//! (`regex`), no I/O — safe to paste contact lists, email threads or exports.

use std::collections::HashSet;

use regex::Regex;
use serde::Serialize;

/// Which contact types to extract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Types {
    Both,
    Emails,
    Phones,
}

impl Types {
    pub fn parse(s: &str) -> Result<Types, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "both" | "all" | "email_and_phone" => Ok(Types::Both),
            "emails" | "email" | "e-mail" | "mail" => Ok(Types::Emails),
            "phones" | "phone" | "telephone" | "tel" | "numbers" => Ok(Types::Phones),
            other => Err(format!(
                "expected types to be 'both', 'emails' or 'phones', got '{other}'"
            )),
        }
    }
    fn wants_email(self) -> bool {
        matches!(self, Types::Both | Types::Emails)
    }
    fn wants_phone(self) -> bool {
        matches!(self, Types::Both | Types::Phones)
    }
}

/// Ordering of the extracted lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    /// Order in which each value first appears in the text.
    FirstSeen,
    /// Ascending case-insensitive/lexicographic order.
    Alphabetical,
}

impl Sort {
    pub fn parse(s: &str) -> Result<Sort, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "first-seen" | "first_seen" | "original" | "text" => Ok(Sort::FirstSeen),
            "alphabetical" | "alpha" | "sorted" | "az" | "a-z" => Ok(Sort::Alphabetical),
            other => Err(format!(
                "expected sort to be 'first-seen' or 'alphabetical', got '{other}'"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Extracted {
    /// Number of email addresses returned.
    pub email_count: usize,
    /// Number of phone numbers returned.
    pub phone_count: usize,
    /// email_count + phone_count.
    pub total: usize,
    /// Email addresses (lowercased). Omitted from JSON when the type wasn't requested / none found.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub emails: Vec<String>,
    /// Phone numbers, kept in their original written form (trimmed). Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub phones: Vec<String>,
}

thread_local! {
    // Pragmatic email matcher: local-part, '@', domain with a real TLD.
    static EMAIL: Regex =
        Regex::new(r"(?i)\b[a-z0-9._%+\-]+@[a-z0-9](?:[a-z0-9\-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9\-]*[a-z0-9])?)*\.[a-z]{2,}\b").unwrap();

    // Phone-number candidates, longest/most-specific forms first. Post-filtered by
    // digit count + a boundary check (see `scan_phones`) to reject partials of longer
    // digit runs (IDs, hashes). Separators are single chars so two space-separated
    // numbers aren't merged into one over-long run.
    static PHONE: Regex = Regex::new(concat!(
        // International: +CC then grouped digits, optional (area).
        r"\+\d{1,3}[\s.\-]?\(?\d{1,4}\)?(?:[\s.\-]?\d{1,4}){1,5}",
        // (area) local, e.g. (415) 555-2671
        r"|\(\d{2,4}\)[\s.\-]?\d{2,4}(?:[\s.\-]?\d{2,4}){1,4}",
        // Grouped with single separators, e.g. 555-123-4567 / 020 7946 0958
        r"|\d{2,4}(?:[\s.\-]\d{2,4}){1,5}",
        // Continuous run of digits (10-15), e.g. 5551234567
        r"|\d{10,15}",
    )).unwrap();
}

/// Extract emails (lowercased, first-seen order) from `text`, deduped when asked.
fn scan_emails(text: &str, dedupe: bool) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    EMAIL.with(|re| {
        for m in re.find_iter(text) {
            let e = m.as_str().to_ascii_lowercase();
            if dedupe && !seen.insert(e.clone()) {
                continue;
            }
            out.push(e);
        }
    });
    out
}

/// Extract phone numbers (original form, first-seen order) from `text`, deduplicated
/// by their normalized digit string when `dedupe` is set.
fn scan_phones(text: &str, dedupe: bool) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    PHONE.with(|re| {
        for m in re.find_iter(text) {
            let (start, end) = (m.start(), m.end());
            // Boundary: reject a candidate that's really the middle/edge of a longer
            // digit run (e.g. a 20-digit id). No lookaround in `regex`, so check here.
            if start > 0 && bytes[start - 1].is_ascii_digit() {
                continue;
            }
            if end < bytes.len() && bytes[end].is_ascii_digit() {
                continue;
            }
            let raw = m.as_str().trim();
            let plus = raw.starts_with('+');
            let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
            // E.164 allows up to 15 digits; require at least 7 to avoid short codes/years.
            if digits.len() < 7 || digits.len() > 15 {
                continue;
            }
            let key = if plus { format!("+{digits}") } else { digits };
            if dedupe && !seen.insert(key) {
                continue;
            }
            out.push(raw.to_string());
        }
    });
    out
}

/// Extract contact info from `text`.
///
/// * `types`  — which categories to pull.
/// * `dedupe` — remove duplicate values (emails case-insensitively, phones by digits).
/// * `sort`   — ordering of each returned list.
pub fn extract(text: &str, types: Types, dedupe: bool, sort: Sort) -> Extracted {
    let mut emails = if types.wants_email() {
        scan_emails(text, dedupe)
    } else {
        Vec::new()
    };

    // Mask emails before the phone scan so long digit runs inside a local-part
    // (e.g. 1234567@x.com) aren't misread as phone numbers.
    let mut phones = if types.wants_phone() {
        let masked = EMAIL.with(|re| re.replace_all(text, " ").into_owned());
        scan_phones(&masked, dedupe)
    } else {
        Vec::new()
    };

    if sort == Sort::Alphabetical {
        emails.sort();
        phones.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));
    }

    Extracted {
        email_count: emails.len(),
        phone_count: phones.len(),
        total: emails.len() + phones.len(),
        emails,
        phones,
    }
}

/// String-keyed wrapper over [`extract`] that parses `types`/`sort` (used by the
/// chat skill block, which returns the structured [`Extracted`] value as JSON).
pub fn extract_str(text: &str, types: &str, dedupe: bool, sort: &str) -> Result<Extracted, String> {
    let types = Types::parse(types)?;
    let sort = Sort::parse(sort)?;
    Ok(extract(text, types, dedupe, sort))
}

/// Human-readable rendering (used by the page).
pub fn render(text: &str, types: &str, dedupe: bool, sort: &str) -> Result<String, String> {
    let types = Types::parse(types)?;
    let sort = Sort::parse(sort)?;
    let r = extract(text, types, dedupe, sort);
    if r.total == 0 {
        return Ok("No contact information found.".to_string());
    }
    let mut out = format!(
        "{} item(s): {} email(s), {} phone(s)\n",
        r.total, r.email_count, r.phone_count
    );
    if !r.emails.is_empty() {
        out.push_str(&format!("\nEmails ({}):\n", r.email_count));
        for e in &r.emails {
            out.push_str(&format!("  {e}\n"));
        }
    }
    if !r.phones.is_empty() {
        out.push_str(&format!("\nPhones ({}):\n", r.phone_count));
        for p in &r.phones {
            out.push_str(&format!("  {p}\n"));
        }
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_both_emails_and_phones() {
        let t = "Reach Alice at alice@corp.com or call +1 415 555 2671. \
                 Bob: bob@corp.com, (212) 555-0199.";
        let r = extract(t, Types::Both, true, Sort::FirstSeen);
        assert_eq!(r.emails, vec!["alice@corp.com", "bob@corp.com"]);
        assert_eq!(r.phones, vec!["+1 415 555 2671", "(212) 555-0199"]);
        assert_eq!(r.email_count, 2);
        assert_eq!(r.phone_count, 2);
        assert_eq!(r.total, 4);
    }

    #[test]
    fn dedupes_emails_case_insensitively_and_phones_by_digits() {
        let t = "a@x.com A@X.COM 555-123-4567 (555) 123 4567";
        let r = extract(t, Types::Both, true, Sort::FirstSeen);
        assert_eq!(r.emails, vec!["a@x.com"]);
        assert_eq!(r.phones, vec!["555-123-4567"]); // same digits -> one entry
    }

    #[test]
    fn dedupe_false_keeps_duplicates() {
        let t = "a@x.com a@x.com 555-123-4567, 555-123-4567";
        let r = extract(t, Types::Both, false, Sort::FirstSeen);
        assert_eq!(r.email_count, 2);
        assert_eq!(r.phone_count, 2);
    }

    #[test]
    fn types_filter() {
        let t = "a@x.com +44 20 7946 0958";
        let emails_only = extract(t, Types::Emails, true, Sort::FirstSeen);
        assert_eq!(emails_only.emails, vec!["a@x.com"]);
        assert!(emails_only.phones.is_empty());
        let phones_only = extract(t, Types::Phones, true, Sort::FirstSeen);
        assert!(phones_only.emails.is_empty());
        assert_eq!(phones_only.phones, vec!["+44 20 7946 0958"]);
    }

    #[test]
    fn alphabetical_sort() {
        let t = "zed@x.com amy@x.com bob@x.com";
        let r = extract(t, Types::Emails, true, Sort::Alphabetical);
        assert_eq!(r.emails, vec!["amy@x.com", "bob@x.com", "zed@x.com"]);
    }

    #[test]
    fn continuous_and_dotted_formats() {
        let r = extract("5551234567 and 555.123.4568", Types::Phones, true, Sort::FirstSeen);
        assert_eq!(r.phones, vec!["5551234567", "555.123.4568"]);
    }

    #[test]
    fn ignores_email_local_part_digits_as_phone() {
        // Long digit run inside an email must not be reported as a phone.
        let r = extract("invoice1234567@billing.example.com", Types::Both, true, Sort::FirstSeen);
        assert_eq!(r.emails, vec!["invoice1234567@billing.example.com"]);
        assert!(r.phones.is_empty(), "phones = {:?}", r.phones);
    }

    #[test]
    fn rejects_too_long_digit_runs() {
        // A 20-digit id is not a phone number.
        let r = extract("id 12345678901234567890 end", Types::Phones, true, Sort::FirstSeen);
        assert!(r.phones.is_empty(), "phones = {:?}", r.phones);
    }

    #[test]
    fn rejects_short_numbers() {
        let r = extract("code 4521 room 12", Types::Phones, true, Sort::FirstSeen);
        assert!(r.phones.is_empty(), "phones = {:?}", r.phones);
    }

    #[test]
    fn empty_when_nothing_matches() {
        let r = extract("just some words here", Types::Both, true, Sort::FirstSeen);
        assert_eq!(r.total, 0);
        assert!(r.emails.is_empty() && r.phones.is_empty());
    }

    #[test]
    fn parse_errors() {
        assert!(Types::parse("bogus").is_err());
        assert!(Sort::parse("bogus").is_err());
        assert!(Types::parse("both").is_ok());
        assert!(Sort::parse("alphabetical").is_ok());
    }

    #[test]
    fn render_summary_and_empty() {
        let out = render("a@x.com +1 415 555 2671", "both", true, "first-seen").unwrap();
        assert!(out.contains("2 item(s): 1 email(s), 1 phone(s)"));
        assert!(out.contains("a@x.com"));
        assert!(out.contains("+1 415 555 2671"));
        assert_eq!(
            render("nothing", "both", true, "first-seen").unwrap(),
            "No contact information found."
        );
        assert!(render("x", "nope", true, "first-seen").is_err());
    }
}
