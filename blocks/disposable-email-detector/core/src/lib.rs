//! disposable-email-detector core — flag whether an email address (or bare
//! domain) uses a known disposable / throwaway-inbox provider.
//!
//! Pure compute, no wafer/wasm-bindgen deps. Shared by the chat skill block and
//! the web page. Never touches the network — the decision is made entirely from
//! a built-in list of well-known disposable-email domains plus a small set of
//! structural heuristics (e.g. an obvious "temp/throwaway/trash" keyword in the
//! domain). It is therefore a fast, private *signal*, not a guarantee: new
//! throwaway services appear constantly, so a `disposable: false` result means
//! "not on the known list", not "definitely a real mailbox".

/// The outcome of checking one address / domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    /// The address (or domain) with surrounding whitespace and a `mailto:` /
    /// `<...>` wrapper stripped — i.e. what was actually checked.
    pub normalized: String,
    /// The domain that was checked (lower-cased), present when one was found.
    pub domain: Option<String>,
    /// Whether the domain matches a known disposable / throwaway provider.
    pub disposable: bool,
    /// Why we flagged it (or did not): a short human-readable explanation.
    pub reason: String,
    /// The specific known disposable domain that matched (the input domain, or
    /// a parent domain when the match was on a subdomain), when any.
    pub matched_domain: Option<String>,
}

/// Curated list of well-known disposable / temporary-inbox email domains. This
/// is deliberately a *representative* set of the most common throwaway services
/// (and their popular alias domains), not an exhaustive registry of every
/// domain on the internet — the goal is a fast, offline, high-precision signal.
const DISPOSABLE_DOMAINS: &[&str] = &[
    // Mailinator and its many alias domains.
    "mailinator.com",
    "mailinator.net",
    "mailinator.org",
    "mailinator2.com",
    "reallymymail.com",
    "binkmail.com",
    "bobmail.info",
    "safetymail.info",
    "sogetthis.com",
    "spamherelots.com",
    "thisisnotmyrealemail.com",
    // Guerrilla Mail family.
    "guerrillamail.com",
    "guerrillamail.net",
    "guerrillamail.org",
    "guerrillamail.biz",
    "guerrillamail.de",
    "guerrillamailblock.com",
    "grr.la",
    "sharklasers.com",
    "spam4.me",
    // 10 Minute Mail / Temp-Mail family.
    "10minutemail.com",
    "10minutemail.net",
    "20minutemail.com",
    "temp-mail.org",
    "tempmail.com",
    "tempmail.net",
    "tempmailo.com",
    "tempr.email",
    "tmail.ws",
    "tmpmail.org",
    "tmpmail.net",
    "tmpeml.com",
    "throwawaymail.com",
    "throwawaymail.net",
    // YOPmail.
    "yopmail.com",
    "yopmail.net",
    "yopmail.fr",
    "cool.fr.nf",
    "jetable.fr.nf",
    "nospam.ze.tc",
    "nomail.xl.cx",
    "mega.zik.dj",
    "speed.1s.fr",
    "courriel.fr.nf",
    "moncourrier.fr.nf",
    "monemail.fr.nf",
    "monmail.fr.nf",
    // Maildrop / Mailnesia / Trashmail family.
    "maildrop.cc",
    "mailnesia.com",
    "trashmail.com",
    "trashmail.net",
    "trashmail.de",
    "trashmail.me",
    "trash-mail.com",
    "trash2009.com",
    "kurzepost.de",
    "objectmail.com",
    "proxymail.eu",
    "rcpt.at",
    "wegwerfmail.de",
    "wegwerfmail.net",
    "wegwerfmail.org",
    // Getnada / Nada / inboxkitten / generators.
    "getnada.com",
    "nada.email",
    "inboxkitten.com",
    "emailondeck.com",
    "fakemail.net",
    "fakeinbox.com",
    "fakemailgenerator.com",
    "dispostable.com",
    "spambog.com",
    "spambog.de",
    "spambog.ru",
    "spamgourmet.com",
    "mailcatch.com",
    "maileater.com",
    "mailexpire.com",
    "mailforspam.com",
    "mailmetrash.com",
    "mailtothis.com",
    "mintemail.com",
    "mt2015.com",
    "mvrht.com",
    "mytemp.email",
    "mohmal.com",
    "moakt.com",
    "tafmail.com",
    "luxusmail.org",
    // Misc well-known throwaway hosts.
    "discard.email",
    "discardmail.com",
    "discardmail.de",
    "dropmail.me",
    "emltmp.com",
    "etranquil.com",
    "spamfree24.org",
    "spamfree24.com",
    "deadaddress.com",
    "byom.de",
    "harakirimail.com",
    "incognitomail.org",
    "instant-mail.de",
    "0-mail.com",
    "0clickemail.com",
    "33mail.com",
    "anonbox.net",
    "anonymbox.com",
    "burnermail.io",
    "armyspy.com",
    "cuvox.de",
    "dayrep.com",
    "einrot.com",
    "fleckens.hu",
    "gustr.com",
    "jourrapide.com",
    "rhyta.com",
    "superrito.com",
    "teleworm.us",
];

/// Lower-cased keywords that, when they appear as a whole *label* in the domain,
/// strongly suggest a throwaway/temporary inbox even if the exact domain is not
/// on the curated list. Kept conservative (matched as a whole label, not a
/// substring) to avoid false positives on real brands.
const THROWAWAY_KEYWORDS: &[&str] = &[
    "tempmail",
    "temp-mail",
    "throwaway",
    "throwawaymail",
    "trashmail",
    "trash-mail",
    "fakemail",
    "fakeinbox",
    "disposable",
    "mailinator",
    "guerrillamail",
    "yopmail",
    "10minutemail",
    "spamgourmet",
];

/// Check one email address (or a bare domain). Never performs I/O.
pub fn detect(input: &str) -> Detection {
    let normalized = strip_wrappers(input);

    if normalized.is_empty() {
        return Detection {
            normalized,
            domain: None,
            disposable: false,
            reason: "no email address or domain provided".to_string(),
            matched_domain: None,
        };
    }

    // Accept either a full address (local@domain) or a bare domain.
    let raw_domain = match normalized.rsplit_once('@') {
        Some((_, d)) => d,
        None => normalized.as_str(),
    };
    let domain = raw_domain.trim().trim_end_matches('.').to_ascii_lowercase();

    if domain.is_empty() || !domain.contains('.') {
        return Detection {
            normalized,
            domain: if domain.is_empty() { None } else { Some(domain) },
            disposable: false,
            reason: "no valid domain found to check (expected name@domain.tld or domain.tld)"
                .to_string(),
            matched_domain: None,
        };
    }

    // 1) Exact match against the curated list.
    if DISPOSABLE_DOMAINS.contains(&domain.as_str()) {
        return Detection {
            normalized,
            domain: Some(domain.clone()),
            disposable: true,
            reason: format!("{domain:?} is a known disposable / throwaway-inbox provider"),
            matched_domain: Some(domain),
        };
    }

    // 2) Subdomain of a known disposable domain (e.g. inbox.mailinator.com).
    if let Some(parent) = DISPOSABLE_DOMAINS
        .iter()
        .find(|d| domain.ends_with(&format!(".{d}")))
    {
        return Detection {
            normalized,
            domain: Some(domain.clone()),
            disposable: true,
            reason: format!("{domain:?} is a subdomain of {parent:?}, a known disposable provider"),
            matched_domain: Some((*parent).to_string()),
        };
    }

    // 3) Heuristic: a throwaway keyword appears as a whole label in the domain.
    for label in domain.split('.') {
        if THROWAWAY_KEYWORDS.contains(&label) {
            return Detection {
                normalized,
                domain: Some(domain.clone()),
                disposable: true,
                reason: format!(
                    "the domain label {label:?} matches a known throwaway-mail keyword"
                ),
                matched_domain: None,
            };
        }
    }

    Detection {
        normalized,
        domain: Some(domain.clone()),
        disposable: false,
        reason: format!(
            "{domain:?} is not on the known-disposable list (this is a signal, not a guarantee)"
        ),
        matched_domain: None,
    }
}

/// Render the detection as a human-readable multi-line report shared by the chat
/// skill and the standalone page so the output is identical everywhere.
pub fn report(input: &str) -> String {
    let d = detect(input);
    let mut out = String::new();
    out.push_str(&format!("Input: {}\n", d.normalized));
    match &d.domain {
        Some(dom) => out.push_str(&format!("Domain: {dom}\n")),
        None => out.push_str("Domain: (none)\n"),
    }
    out.push_str(&format!(
        "Disposable: {}\n",
        if d.disposable { "yes" } else { "no" }
    ));
    if let Some(m) = &d.matched_domain {
        out.push_str(&format!("Matched: {m}\n"));
    }
    out.push_str(&format!("Reason: {}", d.reason));
    out
}

/// Number of domains in the built-in disposable list (used by the page copy and
/// tests).
pub fn known_count() -> usize {
    DISPOSABLE_DOMAINS.len()
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
    fn flags_a_known_disposable_address() {
        let d = detect("someone@mailinator.com");
        assert!(d.disposable, "reason: {}", d.reason);
        assert_eq!(d.domain.as_deref(), Some("mailinator.com"));
        assert_eq!(d.matched_domain.as_deref(), Some("mailinator.com"));
    }

    #[test]
    fn passes_a_real_provider() {
        let d = detect("jane@gmail.com");
        assert!(!d.disposable, "reason: {}", d.reason);
        assert_eq!(d.domain.as_deref(), Some("gmail.com"));
        assert!(d.matched_domain.is_none());
    }

    #[test]
    fn accepts_a_bare_domain() {
        let d = detect("yopmail.com");
        assert!(d.disposable, "reason: {}", d.reason);
        assert_eq!(d.domain.as_deref(), Some("yopmail.com"));
    }

    #[test]
    fn flags_a_subdomain_of_a_disposable_domain() {
        let d = detect("user@inbox.mailinator.com");
        assert!(d.disposable, "reason: {}", d.reason);
        assert_eq!(d.matched_domain.as_deref(), Some("mailinator.com"));
        assert!(d.reason.contains("subdomain"), "{}", d.reason);
    }

    #[test]
    fn is_case_insensitive() {
        let d = detect("User@MailInAtoR.CoM");
        assert!(d.disposable, "reason: {}", d.reason);
        assert_eq!(d.domain.as_deref(), Some("mailinator.com"));
    }

    #[test]
    fn keyword_heuristic_catches_unlisted_tempmail() {
        // not on the curated list but the label "tempmail" is a keyword.
        let d = detect("x@tempmail.example.io");
        assert!(d.disposable, "reason: {}", d.reason);
        assert!(d.matched_domain.is_none());
        assert!(d.reason.contains("keyword"), "{}", d.reason);
    }

    #[test]
    fn keyword_heuristic_does_not_overmatch_substrings() {
        // "contemporary" contains "temp" but is not a whole-label keyword.
        let d = detect("hi@contemporary-art.org");
        assert!(!d.disposable, "reason: {}", d.reason);
    }

    #[test]
    fn strips_mailto_and_angle_brackets() {
        let d = detect("Jane <mailto:jane@guerrillamail.com>");
        assert!(d.disposable, "reason: {}", d.reason);
        assert_eq!(d.domain.as_deref(), Some("guerrillamail.com"));
    }

    #[test]
    fn empty_input_is_not_disposable() {
        let d = detect("   ");
        assert!(!d.disposable);
        assert!(d.domain.is_none());
        assert!(d.reason.contains("no email"), "{}", d.reason);
    }

    #[test]
    fn no_dot_domain_is_reported_invalid() {
        let d = detect("user@localhost");
        assert!(!d.disposable);
        assert!(d.reason.contains("no valid domain"), "{}", d.reason);
    }

    #[test]
    fn trailing_dot_is_tolerated() {
        let d = detect("user@mailinator.com.");
        assert!(d.disposable, "reason: {}", d.reason);
        assert_eq!(d.domain.as_deref(), Some("mailinator.com"));
    }

    #[test]
    fn known_count_is_reasonable() {
        assert!(known_count() > 50, "have {}", known_count());
    }

    #[test]
    fn report_renders_sections_for_disposable() {
        let r = report("a@mailinator.com");
        assert!(r.contains("Input: a@mailinator.com"), "{r}");
        assert!(r.contains("Domain: mailinator.com"), "{r}");
        assert!(r.contains("Disposable: yes"), "{r}");
        assert!(r.contains("Matched: mailinator.com"), "{r}");
    }

    #[test]
    fn report_renders_sections_for_clean() {
        let r = report("a@gmail.com");
        assert!(r.contains("Disposable: no"), "{r}");
        assert!(!r.contains("Matched:"), "{r}");
    }
}
