//! email-normalizer core — canonicalize an email address to its deliverable form.
//!
//! Pure compute, no wafer/wasm-bindgen deps. Shared by the chat skill block and
//! the web page.
//!
//! Normalization steps (all on the parsed `local@domain`):
//!   1. trim surrounding whitespace and a single pair of `< >` (e.g. from a
//!      "Name <addr>" header) and a leading `mailto:`.
//!   2. lowercase the domain (DNS is case-insensitive). The local part is only
//!      lowercased when the provider is known to treat it case-insensitively,
//!      which in practice is every mainstream consumer provider — so by default
//!      we lowercase the local part too (toggle with `lowercase_local = false`).
//!   3. apply provider-specific canonicalization for known domains:
//!        * Gmail (`gmail.com`, `googlemail.com`): strip all `.` from the local
//!          part and drop everything after the first `+`; canonical domain is
//!          `gmail.com`.
//!        * Outlook/Hotmail/Live, Yahoo, iCloud, FastMail, ProtonMail and other
//!          plus-tag providers: drop the `+tag` sub-address. Dots are preserved
//!          for these (only Gmail ignores dots).
//!   4. report whether the address had a sub-address tag, the tag itself, and a
//!      human-readable provider name.

/// The recognised provider an address belongs to, used to pick the right
/// canonicalization rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    /// Gmail / Google Workspace consumer (`gmail.com`, `googlemail.com`).
    Gmail,
    /// Microsoft consumer (`outlook.com`, `hotmail.com`, `live.com`, `msn.com`).
    Outlook,
    /// Yahoo (`yahoo.com`, `ymail.com`, `rocketmail.com`).
    Yahoo,
    /// Apple iCloud (`icloud.com`, `me.com`, `mac.com`).
    Icloud,
    /// Fastmail (`fastmail.com`, `fastmail.fm`).
    Fastmail,
    /// Proton Mail (`proton.me`, `protonmail.com`, `pm.me`).
    Proton,
    /// Any other domain — generic rules only.
    Other,
}

impl Provider {
    /// Human-readable label for the report.
    pub fn label(self) -> &'static str {
        match self {
            Provider::Gmail => "Gmail",
            Provider::Outlook => "Outlook",
            Provider::Yahoo => "Yahoo",
            Provider::Icloud => "iCloud",
            Provider::Fastmail => "Fastmail",
            Provider::Proton => "Proton Mail",
            Provider::Other => "Other",
        }
    }

    /// Whether this provider ignores `.` in the local part (only Gmail does).
    fn ignores_dots(self) -> bool {
        matches!(self, Provider::Gmail)
    }

    /// Classify a (already-lowercased) domain into a provider.
    fn from_domain(domain: &str) -> Provider {
        match domain {
            "gmail.com" | "googlemail.com" => Provider::Gmail,
            "outlook.com" | "hotmail.com" | "live.com" | "msn.com" | "hotmail.co.uk"
            | "outlook.co.uk" | "live.co.uk" => Provider::Outlook,
            "yahoo.com" | "ymail.com" | "rocketmail.com" | "yahoo.co.uk" => Provider::Yahoo,
            "icloud.com" | "me.com" | "mac.com" => Provider::Icloud,
            "fastmail.com" | "fastmail.fm" => Provider::Fastmail,
            "proton.me" | "protonmail.com" | "pm.me" => Provider::Proton,
            _ => Provider::Other,
        }
    }

    /// The canonical domain to emit (Gmail folds `googlemail.com` → `gmail.com`).
    fn canonical_domain(self, domain: &str) -> String {
        match self {
            Provider::Gmail => "gmail.com".to_string(),
            _ => domain.to_string(),
        }
    }
}

/// The separator that introduces a sub-address ("plus") tag.
const TAG_SEP: char = '+';

/// The result of normalizing one email address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Normalized {
    /// The cleaned input as parsed `local@domain` before provider rules — only
    /// trimming + case folding applied. Useful for "what you typed, tidied up".
    pub cleaned: String,
    /// The canonical, deliverable form after provider-specific rules.
    pub canonical: String,
    /// The local part (before `@`) of the canonical address.
    pub local: String,
    /// The domain (after `@`) of the canonical address.
    pub domain: String,
    /// The recognised provider.
    pub provider: Provider,
    /// Whether the original address carried a `+tag` sub-address.
    pub had_tag: bool,
    /// The sub-address tag that was stripped (without the leading `+`), if any.
    pub tag: Option<String>,
    /// Whether any dots were removed from the local part (Gmail only).
    pub removed_dots: bool,
}

/// Normalize one email address to its canonical deliverable form.
///
/// - `lowercase_local`: when `true` (the default) the local part is lowercased.
///   Set it `false` to preserve the local-part case for the rare server that is
///   case-sensitive (RFC 5321 permits this, though virtually no provider does).
///
/// Returns `Err` with a human-readable message when the input is not a single
/// syntactically plausible `local@domain` address.
pub fn normalize(input: &str, lowercase_local: bool) -> Result<Normalized, String> {
    let raw = strip_wrappers(input);
    if raw.is_empty() {
        return Err("no email address provided".to_string());
    }

    // Exactly one '@' splits local from domain. (Quoted local parts containing
    // '@' are not supported — they are vanishingly rare and not deliverable in
    // practice for the consumer providers this tool targets.)
    let at = raw
        .find('@')
        .ok_or_else(|| format!("invalid email {raw:?}: missing '@' separator"))?;
    if raw[at + 1..].contains('@') {
        return Err(format!("invalid email {raw:?}: more than one '@'"));
    }
    let (local_raw, domain_raw) = (&raw[..at], &raw[at + 1..]);
    if local_raw.is_empty() {
        return Err(format!("invalid email {raw:?}: empty local part before '@'"));
    }
    validate_domain(domain_raw)?;

    let domain = domain_raw.to_ascii_lowercase();
    let local_cased = if lowercase_local {
        local_raw.to_ascii_lowercase()
    } else {
        local_raw.to_string()
    };

    let provider = Provider::from_domain(&domain);

    // The "cleaned" form is just trim + case folding, no provider rules.
    let cleaned = format!("{local_cased}@{domain}");

    // Split off the +tag sub-address (every supported provider honours it).
    let (mut local, tag) = match local_cased.split_once(TAG_SEP) {
        Some((base, t)) => (base.to_string(), Some(t.to_string())),
        None => (local_cased.clone(), None),
    };

    // Gmail ignores dots in the local part.
    let removed_dots = provider.ignores_dots() && local.contains('.');
    if provider.ignores_dots() {
        local = local.replace('.', "");
    }

    if local.is_empty() {
        return Err(format!(
            "invalid email {raw:?}: local part is empty after normalization"
        ));
    }

    let canonical_domain = provider.canonical_domain(&domain);
    let canonical = format!("{local}@{canonical_domain}");

    Ok(Normalized {
        cleaned,
        canonical,
        local,
        domain: canonical_domain,
        provider,
        had_tag: tag.is_some(),
        tag,
        removed_dots,
    })
}

/// Normalize `input` and render a human-readable multi-line report. This is the
/// shared text surface used by both the chat skill and the standalone page so
/// the output is identical everywhere. Returns `Err` on an invalid address.
pub fn report(input: &str, lowercase_local: bool) -> Result<String, String> {
    let n = normalize(input, lowercase_local)?;
    let mut out = String::new();
    out.push_str(&format!("Canonical: {}\n", n.canonical));
    out.push_str(&format!("Local part: {}\n", n.local));
    out.push_str(&format!("Domain: {}\n", n.domain));
    out.push_str(&format!("Provider: {}\n", n.provider.label()));
    if n.had_tag {
        out.push_str(&format!(
            "Sub-address tag: +{} (removed)\n",
            n.tag.as_deref().unwrap_or("")
        ));
    } else {
        out.push_str("Sub-address tag: none\n");
    }
    out.push_str(&format!(
        "Dots removed: {}\n",
        if n.removed_dots { "yes" } else { "no" }
    ));
    out.push_str(&format!("Cleaned (case/trim only): {}", n.cleaned));
    Ok(out)
}

/// Strip a leading `mailto:`, surrounding whitespace, and one pair of angle
/// brackets / display-name wrapper (`Some Name <addr>` → `addr`).
fn strip_wrappers(input: &str) -> String {
    let mut s = input.trim();
    // "Name <addr>" — take what's inside the last <...>.
    if let (Some(lt), Some(gt)) = (s.rfind('<'), s.rfind('>')) {
        if lt < gt {
            s = s[lt + 1..gt].trim();
        }
    }
    // mailto: scheme prefix.
    if let Some(rest) = s.strip_prefix("mailto:") {
        s = rest.trim();
    }
    s.to_string()
}

/// Reject obviously-malformed domains. Keeps the parser strict enough to catch
/// typos without re-implementing full RFC 5322 — at least one dot, no leading/
/// trailing dot or hyphen, no empty labels, only `[A-Za-z0-9.-]`.
fn validate_domain(domain: &str) -> Result<(), String> {
    if domain.is_empty() {
        return Err("invalid email: empty domain after '@'".to_string());
    }
    if !domain.contains('.') {
        return Err(format!(
            "invalid email: domain {domain:?} has no dot (not a fully-qualified domain)"
        ));
    }
    if domain.starts_with('.') || domain.ends_with('.') {
        return Err(format!(
            "invalid email: domain {domain:?} starts or ends with '.'"
        ));
    }
    for label in domain.split('.') {
        if label.is_empty() {
            return Err(format!("invalid email: domain {domain:?} has an empty label"));
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err(format!(
                "invalid email: domain label {label:?} starts or ends with '-'"
            ));
        }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(format!(
                "invalid email: domain label {label:?} has an invalid character"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gmail_strips_dots_and_tag_and_folds_domain() {
        let n = normalize("John.Doe+newsletter@googlemail.com", true).unwrap();
        assert_eq!(n.canonical, "johndoe@gmail.com");
        assert_eq!(n.local, "johndoe");
        assert_eq!(n.domain, "gmail.com");
        assert_eq!(n.provider, Provider::Gmail);
        assert!(n.had_tag);
        assert_eq!(n.tag.as_deref(), Some("newsletter"));
        assert!(n.removed_dots);
    }

    #[test]
    fn outlook_keeps_dots_drops_tag() {
        let n = normalize("First.Last+promo@Outlook.com", true).unwrap();
        assert_eq!(n.canonical, "first.last@outlook.com");
        assert_eq!(n.provider, Provider::Outlook);
        assert!(n.had_tag);
        assert_eq!(n.tag.as_deref(), Some("promo"));
        assert!(!n.removed_dots);
    }

    #[test]
    fn generic_domain_drops_tag_keeps_dots_and_domain() {
        let n = normalize("a.b.c+tag@Example.CO.UK", true).unwrap();
        assert_eq!(n.canonical, "a.b.c@example.co.uk");
        assert_eq!(n.provider, Provider::Other);
        assert_eq!(n.tag.as_deref(), Some("tag"));
        assert!(!n.removed_dots);
    }

    #[test]
    fn strips_display_name_and_mailto_and_whitespace() {
        let n = normalize("  Jane Roe <mailto:Jane.Roe@gmail.com>  ", true).unwrap();
        assert_eq!(n.canonical, "janeroe@gmail.com");
    }

    #[test]
    fn preserve_local_case_when_disabled() {
        let n = normalize("Jane.Roe@example.com", false).unwrap();
        assert_eq!(n.canonical, "Jane.Roe@example.com");
        // domain is always lowercased.
        let n2 = normalize("x@Example.COM", false).unwrap();
        assert_eq!(n2.canonical, "x@example.com");
    }

    #[test]
    fn no_tag_when_absent() {
        let n = normalize("user@yahoo.com", true).unwrap();
        assert!(!n.had_tag);
        assert_eq!(n.tag, None);
        assert_eq!(n.provider, Provider::Yahoo);
    }

    #[test]
    fn cleaned_keeps_dots_and_tag_for_gmail() {
        // cleaned is pre-provider: only case folded, tag + dots preserved.
        let n = normalize("John.Doe+x@Gmail.com", true).unwrap();
        assert_eq!(n.cleaned, "john.doe+x@gmail.com");
        assert_eq!(n.canonical, "johndoe@gmail.com");
    }

    #[test]
    fn rejects_missing_at() {
        let err = normalize("not-an-email", true).unwrap_err();
        assert!(err.contains("missing '@'"), "got: {err}");
    }

    #[test]
    fn rejects_double_at() {
        let err = normalize("a@b@example.com", true).unwrap_err();
        assert!(err.contains("more than one '@'"), "got: {err}");
    }

    #[test]
    fn rejects_empty_local() {
        let err = normalize("@example.com", true).unwrap_err();
        assert!(err.contains("empty local part"), "got: {err}");
    }

    #[test]
    fn rejects_domain_without_dot() {
        let err = normalize("a@localhost", true).unwrap_err();
        assert!(err.contains("no dot"), "got: {err}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = normalize("   ", true).unwrap_err();
        assert!(err.contains("no email"), "got: {err}");
    }

    #[test]
    fn rejects_bad_domain_label() {
        let err = normalize("a@exa_mple.com", true).unwrap_err();
        assert!(err.contains("invalid character"), "got: {err}");
        let err2 = normalize("a@ex..ample.com", true).unwrap_err();
        assert!(err2.contains("empty label"), "got: {err2}");
    }

    #[test]
    fn report_renders_all_fields() {
        let r = report("John.Doe+news@gmail.com", true).unwrap();
        assert!(r.contains("Canonical: johndoe@gmail.com"), "got: {r}");
        assert!(r.contains("Provider: Gmail"), "got: {r}");
        assert!(r.contains("Sub-address tag: +news (removed)"), "got: {r}");
        assert!(r.contains("Dots removed: yes"), "got: {r}");
    }

    #[test]
    fn report_propagates_errors() {
        assert!(report("nope", true).is_err());
    }

    #[test]
    fn gmail_only_tag_with_dots_collapses() {
        // local that is dots collapses correctly for Gmail.
        let n = normalize("j.d@gmail.com", true).unwrap();
        assert_eq!(n.canonical, "jd@gmail.com");
        // a degenerate all-removed local is an error.
        let err = normalize(".+tag@gmail.com", true).unwrap_err();
        assert!(err.contains("empty after normalization"), "got: {err}");
    }
}
