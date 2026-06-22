//! email-validator core — validate an email address against RFC 5321/5322
//! syntax rules and flag common typos and formatting issues.
//!
//! Pure compute, no wafer/wasm-bindgen deps. Shared by the chat skill block and
//! the web page.
//!
//! This is a *syntax* validator (it never touches the network — no DNS/MX or
//! SMTP probing). It parses `local@domain`, checks both parts against the
//! practical subset of RFC 5321/5322 that real mail systems accept, and then
//! surfaces a set of human-readable warnings for things that are technically
//! valid but almost certainly a mistake: a misspelled popular domain
//! (`gmial.com` → `gmail.com`), a misspelled TLD (`.con` → `.com`), a missing
//! dot in the domain, trailing/leading whitespace, an over-long local part, a
//! consecutive-dot run, an all-numeric TLD, and so on.

/// The outcome of validating one email address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Validation {
    /// The address with surrounding whitespace and a `mailto:` / `<...>`
    /// wrapper stripped — i.e. what was actually validated.
    pub normalized: String,
    /// Whether the address is syntactically valid (no hard errors).
    pub valid: bool,
    /// The local part (before `@`), present when an `@` was found.
    pub local: Option<String>,
    /// The domain (after `@`), present when an `@` was found.
    pub domain: Option<String>,
    /// Hard syntax errors. Non-empty ⇒ `valid == false`.
    pub errors: Vec<String>,
    /// Soft warnings — the address parses, but something looks suspicious
    /// (likely typo, deprecated form, deliverability risk).
    pub warnings: Vec<String>,
    /// A best-guess corrected address when a likely typo was detected
    /// (e.g. a misspelled provider domain or TLD). `None` when nothing to fix.
    pub suggestion: Option<String>,
}

/// Maximum length of the whole address (the addr-spec itself is 254 chars).
const MAX_TOTAL: usize = 254;
/// Maximum length of the local part (RFC 5321 §4.5.3.1.1).
const MAX_LOCAL: usize = 64;
/// Maximum length of a single DNS label (RFC 1035).
const MAX_LABEL: usize = 63;
/// Maximum length of the domain (RFC 1035, 253 as text).
const MAX_DOMAIN: usize = 253;

/// Validate one email address. Never performs I/O.
pub fn validate(input: &str) -> Validation {
    let normalized = strip_wrappers(input);
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut suggestion: Option<String> = None;

    if input != input.trim() {
        warnings.push("the address had surrounding whitespace (trimmed)".to_string());
    }

    if normalized.is_empty() {
        errors.push("no email address provided".to_string());
        return Validation {
            normalized,
            valid: false,
            local: None,
            domain: None,
            errors,
            warnings,
            suggestion,
        };
    }

    if normalized.len() > MAX_TOTAL {
        errors.push(format!(
            "address is {} characters, over the {MAX_TOTAL}-character maximum",
            normalized.len()
        ));
    }

    // Split on the LAST '@' so a quoted local part with an '@' is tolerated; we
    // still reject unquoted multi-'@'.
    let at_count = normalized.matches('@').count();
    let (local, domain) = match normalized.rsplit_once('@') {
        Some((l, d)) => (l.to_string(), d.to_string()),
        None => {
            errors.push("missing '@' separator".to_string());
            return Validation {
                normalized,
                valid: false,
                local: None,
                domain: None,
                errors,
                warnings,
                suggestion,
            };
        }
    };

    let local_quoted = local.starts_with('"') && local.ends_with('"') && local.len() >= 2;
    if at_count > 1 && !local_quoted {
        errors.push("more than one '@' (the local part must be quoted to contain '@')".to_string());
    }

    validate_local(&local, local_quoted, &mut errors, &mut warnings);
    let domain_fix = validate_domain(&domain, &mut errors, &mut warnings);

    if let Some(fixed_domain) = domain_fix {
        if fixed_domain != domain {
            suggestion = Some(format!("{local}@{fixed_domain}"));
        }
    }

    let valid = errors.is_empty();
    Validation {
        normalized,
        valid,
        local: Some(local),
        domain: Some(domain),
        errors,
        warnings,
        suggestion,
    }
}

/// Validate the local part (before `@`).
fn validate_local(local: &str, quoted: bool, errors: &mut Vec<String>, warnings: &mut Vec<String>) {
    if local.is_empty() {
        errors.push("empty local part before '@'".to_string());
        return;
    }
    if local.len() > MAX_LOCAL {
        errors.push(format!(
            "local part is {} characters, over the {MAX_LOCAL}-character maximum",
            local.len()
        ));
    }

    if quoted {
        warnings.push("the local part is a quoted string — many systems reject it".to_string());
        return;
    }

    if local.starts_with('.') {
        errors.push("local part starts with a dot".to_string());
    }
    if local.ends_with('.') {
        errors.push("local part ends with a dot".to_string());
    }
    if local.contains("..") {
        errors.push("local part has consecutive dots".to_string());
    }

    // RFC 5322 atext: A-Za-z0-9 and !#$%&'*+-/=?^_`{|}~ plus '.' as a separator.
    const ATEXT_SPECIAL: &str = "!#$%&'*+-/=?^_`{|}~";
    for c in local.chars() {
        let ok = c.is_ascii_alphanumeric() || c == '.' || ATEXT_SPECIAL.contains(c);
        if !ok {
            if c == ' ' {
                errors.push("local part contains a space (quote it or remove it)".to_string());
            } else if !c.is_ascii() {
                errors.push(format!(
                    "local part contains a non-ASCII character {c:?} (not allowed without internationalized-email support)"
                ));
            } else {
                errors.push(format!("local part contains an invalid character {c:?}"));
            }
        }
    }
}

/// Validate the domain (after `@`). Returns a suggested corrected domain when a
/// likely typo (misspelled provider or TLD) was detected.
fn validate_domain(
    domain: &str,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Option<String> {
    if domain.is_empty() {
        errors.push("empty domain after '@'".to_string());
        return None;
    }

    // IP-address literal: user@[192.0.2.1] — valid but unusual.
    if domain.starts_with('[') && domain.ends_with(']') {
        warnings.push("the domain is an IP-address literal — valid but very unusual".to_string());
        return None;
    }

    if domain.len() > MAX_DOMAIN {
        errors.push(format!(
            "domain is {} characters, over the {MAX_DOMAIN}-character maximum",
            domain.len()
        ));
    }

    let lower = domain.to_ascii_lowercase();
    let mut had_structural_error = false;

    if !domain.contains('.') {
        errors.push(format!(
            "domain {domain:?} has no dot (not a fully-qualified domain name)"
        ));
        had_structural_error = true;
    }
    if domain.starts_with('.') || domain.ends_with('.') {
        errors.push(format!("domain {domain:?} starts or ends with a dot"));
        had_structural_error = true;
    }
    if domain.contains("..") {
        errors.push(format!("domain {domain:?} has consecutive dots (empty label)"));
        had_structural_error = true;
    }
    if domain.starts_with('-') || domain.ends_with('-') {
        errors.push(format!("domain {domain:?} starts or ends with a hyphen"));
        had_structural_error = true;
    }

    for label in domain.split('.') {
        if label.is_empty() {
            continue;
        }
        if label.len() > MAX_LABEL {
            errors.push(format!("domain label {label:?} is over {MAX_LABEL} characters"));
        }
        if label.starts_with('-') || label.ends_with('-') {
            errors.push(format!("domain label {label:?} starts or ends with a hyphen"));
        }
        for c in label.chars() {
            if !(c.is_ascii_alphanumeric() || c == '-') {
                if !c.is_ascii() {
                    errors.push(format!(
                        "domain contains a non-ASCII character {c:?} (use the punycode 'xn--' form)"
                    ));
                } else {
                    errors.push(format!("domain label {label:?} has an invalid character {c:?}"));
                }
                break;
            }
        }
    }

    // ----- soft typo / deliverability warnings (only if structurally sane) -----
    let mut suggestion: Option<String> = None;
    if !had_structural_error {
        if let Some(tld) = lower.rsplit('.').next() {
            if tld.chars().all(|c| c.is_ascii_digit()) {
                warnings.push(format!("the top-level domain {tld:?} is all digits — likely a typo"));
            }
            if let Some(fixed_tld) = fix_tld(tld) {
                let base = &lower[..lower.len() - tld.len()];
                let fixed = format!("{base}{fixed_tld}");
                warnings.push(format!(
                    "{tld:?} looks like a misspelling of {fixed_tld:?} — did you mean {fixed:?}?"
                ));
                suggestion = Some(fixed);
            }
        }

        if let Some(fixed) = fix_domain(&lower) {
            warnings.push(format!(
                "{lower:?} looks like a misspelling of {fixed:?} — did you mean {fixed:?}?"
            ));
            suggestion = Some(fixed);
        }
    }

    suggestion
}

/// Common misspellings of popular consumer-mail domains → the correct domain.
fn fix_domain(domain: &str) -> Option<String> {
    let fixed = match domain {
        "gmial.com" | "gmai.com" | "gmal.com" | "gmil.com" | "gnail.com" | "gmaill.com"
        | "gamil.com" | "gmali.com" | "gmsil.com" => "gmail.com",
        "hotmial.com" | "hotmai.com" | "hotmal.com" | "hotmil.com" | "hatmail.com"
        | "hotnail.com" | "homail.com" => "hotmail.com",
        "outlok.com" | "outloo.com" | "outook.com" | "outliook.com" | " outlook.com" => {
            "outlook.com"
        }
        "yaho.com" | "yhoo.com" | "yahooo.com" | "yaoo.com" | "yahho.com" => "yahoo.com",
        "iclod.com" | "iclould.com" | "icoud.com" | "iclooud.com" => "icloud.com",
        "protonmai.com" | "protomail.com" | "protonmial.com" => "protonmail.com",
        _ => return None,
    };
    Some(fixed.to_string())
}

/// Common misspellings of popular TLDs → the correct TLD (without the dot).
fn fix_tld(tld: &str) -> Option<String> {
    let fixed = match tld {
        "con" | "cmo" | "vom" | "comm" | "ocm" | "cpm" | "xom" | "cim" | "vcom" => "com",
        "ner" | "nett" => "net",
        "ogr" | "orgg" => "org",
        "edi" => "edu",
        _ => return None,
    };
    Some(fixed.to_string())
}

/// Render the validation as a human-readable multi-line report shared by the
/// chat skill and the standalone page so the output is identical everywhere.
pub fn report(input: &str) -> String {
    let v = validate(input);
    let mut out = String::new();
    out.push_str(&format!("Address: {}\n", v.normalized));
    out.push_str(&format!("Valid: {}\n", if v.valid { "yes" } else { "no" }));
    if let Some(l) = &v.local {
        out.push_str(&format!("Local part: {l}\n"));
    }
    if let Some(d) = &v.domain {
        out.push_str(&format!("Domain: {d}\n"));
    }
    if v.errors.is_empty() {
        out.push_str("Errors: none\n");
    } else {
        out.push_str("Errors:\n");
        for e in &v.errors {
            out.push_str(&format!("  - {e}\n"));
        }
    }
    if v.warnings.is_empty() {
        out.push_str("Warnings: none\n");
    } else {
        out.push_str("Warnings:\n");
        for w in &v.warnings {
            out.push_str(&format!("  - {w}\n"));
        }
    }
    match &v.suggestion {
        Some(s) => out.push_str(&format!("Suggestion: {s}")),
        None => out.push_str("Suggestion: none"),
    }
    out
}

/// Strip a leading `mailto:`, surrounding whitespace, and one `Name <addr>` /
/// angle-bracket wrapper.
fn strip_wrappers(input: &str) -> String {
    let mut s = input.trim();
    if let (Some(lt), Some(gt)) = (s.rfind('<'), s.rfind('>')) {
        if lt < gt {
            s = s[lt + 1..gt].trim();
        }
    }
    if let Some(rest) = s.strip_prefix("mailto:") {
        s = rest.trim();
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_plain_valid_address() {
        let v = validate("user@example.com");
        assert!(v.valid, "errors: {:?}", v.errors);
        assert_eq!(v.local.as_deref(), Some("user"));
        assert_eq!(v.domain.as_deref(), Some("example.com"));
        assert!(v.errors.is_empty());
        assert!(v.suggestion.is_none());
    }

    #[test]
    fn accepts_plus_tag_and_specials() {
        let v = validate("john.doe+news_letter@sub.example.co.uk");
        assert!(v.valid, "errors: {:?}", v.errors);
        let v2 = validate("a!#$%&'*+-/=?^_`{|}~b@example.com");
        assert!(v2.valid, "errors: {:?}", v2.errors);
    }

    #[test]
    fn rejects_missing_at() {
        let v = validate("not-an-email");
        assert!(!v.valid);
        assert!(v.errors.iter().any(|e| e.contains("missing '@'")), "{:?}", v.errors);
    }

    #[test]
    fn rejects_double_at() {
        let v = validate("a@b@example.com");
        assert!(!v.valid);
        assert!(v.errors.iter().any(|e| e.contains("more than one '@'")), "{:?}", v.errors);
    }

    #[test]
    fn rejects_empty_local() {
        let v = validate("@example.com");
        assert!(!v.valid);
        assert!(v.errors.iter().any(|e| e.contains("empty local part")), "{:?}", v.errors);
    }

    #[test]
    fn rejects_empty_domain() {
        let v = validate("user@");
        assert!(!v.valid);
        assert!(v.errors.iter().any(|e| e.contains("empty domain")), "{:?}", v.errors);
    }

    #[test]
    fn rejects_domain_without_dot() {
        let v = validate("user@localhost");
        assert!(!v.valid);
        assert!(v.errors.iter().any(|e| e.contains("no dot")), "{:?}", v.errors);
    }

    #[test]
    fn rejects_consecutive_dots() {
        let v = validate("a..b@example.com");
        assert!(!v.valid);
        assert!(v.errors.iter().any(|e| e.contains("consecutive dots")), "{:?}", v.errors);
        let v2 = validate("user@exam..ple.com");
        assert!(!v2.valid);
        assert!(v2.errors.iter().any(|e| e.contains("consecutive dots")), "{:?}", v2.errors);
    }

    #[test]
    fn rejects_leading_trailing_dot_in_local() {
        assert!(!validate(".user@example.com").valid);
        assert!(!validate("user.@example.com").valid);
    }

    #[test]
    fn rejects_invalid_chars() {
        let v = validate("us er@example.com");
        assert!(!v.valid);
        assert!(v.errors.iter().any(|e| e.contains("space")), "{:?}", v.errors);
        let v2 = validate("user@exa_mple.com");
        assert!(!v2.valid);
        assert!(v2.errors.iter().any(|e| e.contains("invalid character")), "{:?}", v2.errors);
    }

    #[test]
    fn rejects_hyphen_edges_in_domain() {
        assert!(!validate("user@-example.com").valid);
        assert!(!validate("user@example-.com").valid);
    }

    #[test]
    fn flags_gmail_typo_with_suggestion() {
        let v = validate("user@gmial.com");
        assert!(v.valid, "structurally valid: {:?}", v.errors);
        assert_eq!(v.suggestion.as_deref(), Some("user@gmail.com"));
        assert!(v.warnings.iter().any(|w| w.contains("gmail.com")), "{:?}", v.warnings);
    }

    #[test]
    fn flags_tld_typo_with_suggestion() {
        let v = validate("user@example.con");
        assert!(v.valid, "structurally valid: {:?}", v.errors);
        assert_eq!(v.suggestion.as_deref(), Some("user@example.com"));
    }

    #[test]
    fn flags_numeric_tld() {
        let v = validate("user@example.123");
        assert!(v.warnings.iter().any(|w| w.contains("all digits")), "{:?}", v.warnings);
    }

    #[test]
    fn flags_whitespace_trim() {
        let v = validate("  user@example.com  ");
        assert!(v.valid);
        assert_eq!(v.normalized, "user@example.com");
        assert!(v.warnings.iter().any(|w| w.contains("whitespace")), "{:?}", v.warnings);
    }

    #[test]
    fn strips_mailto_and_angle_brackets() {
        let v = validate("Jane Roe <mailto:jane@example.com>");
        assert!(v.valid, "errors: {:?}", v.errors);
        assert_eq!(v.normalized, "jane@example.com");
    }

    #[test]
    fn accepts_ip_literal_with_warning() {
        let v = validate("user@[192.0.2.1]");
        assert!(v.valid, "errors: {:?}", v.errors);
        assert!(v.warnings.iter().any(|w| w.contains("IP-address literal")), "{:?}", v.warnings);
    }

    #[test]
    fn rejects_overlong_local() {
        let long = "a".repeat(65);
        let v = validate(&format!("{long}@example.com"));
        assert!(!v.valid);
        assert!(v.errors.iter().any(|e| e.contains("64-character maximum")), "{:?}", v.errors);
    }

    #[test]
    fn empty_input_is_invalid() {
        let v = validate("   ");
        assert!(!v.valid);
        assert!(v.errors.iter().any(|e| e.contains("no email")), "{:?}", v.errors);
    }

    #[test]
    fn report_renders_sections() {
        let r = report("user@gmial.com");
        assert!(r.contains("Address: user@gmial.com"), "{r}");
        assert!(r.contains("Valid: yes"), "{r}");
        assert!(r.contains("Suggestion: user@gmail.com"), "{r}");
    }

    #[test]
    fn report_invalid_shows_errors() {
        let r = report("bad");
        assert!(r.contains("Valid: no"), "{r}");
        assert!(r.contains("missing '@'"), "{r}");
    }
}
