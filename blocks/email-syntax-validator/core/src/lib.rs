//! email-syntax-validator core — validate one address's syntax and flag
//! disposable / role-based mailboxes.
//!
//! Pure compute, no wafer/wasm-bindgen deps. Shared by the chat skill block,
//! the CLI and the web page. It combines three offline signals in one call:
//!   1. practical RFC 5321/5322 syntax (reuses `email-validator`), including
//!      soft warnings and typo suggestions;
//!   2. a bundled disposable / throwaway-inbox domain list (reuses
//!      `disposable-email-detector`);
//!   3. a role-based local-part list (`admin`, `info`, `support`, …).
//!
//! It never touches the network — no DNS, MX or SMTP probing — so a `pass`
//! verdict means "well-formed and not on the bundled risk lists", not "this
//! mailbox exists".

use gizza_ai_disposable_email_detector_core::detect;
use gizza_ai_email_validator_core::validate;

/// Local parts that usually reach a team, alias or automated queue rather than
/// one person. Compared case-insensitively, with `_` folded to `-`.
const ROLE_LOCAL_PARTS: &[&str] = &[
    "abuse",
    "admin",
    "administrator",
    "billing",
    "compliance",
    "contact",
    "devnull",
    "dns",
    "help",
    "hello",
    "hostmaster",
    "info",
    "inquiries",
    "jobs",
    "legal",
    "mail",
    "marketing",
    "news",
    "newsletter",
    "no-reply",
    "noreply",
    "office",
    "operations",
    "postmaster",
    "press",
    "privacy",
    "recruiting",
    "root",
    "sales",
    "security",
    "support",
    "team",
    "webmaster",
];

/// Whether a local part is a well-known role / group alias.
fn role_local(local: &str) -> bool {
    let normalized = local.trim().to_ascii_lowercase().replace('_', "-");
    ROLE_LOCAL_PARTS.contains(&normalized.as_str())
}

/// Number of role local parts in the bundled list (surfaced in the page copy).
pub fn role_count() -> usize {
    ROLE_LOCAL_PARTS.len()
}

/// Validate one address and render it in `format` (`report`, `summary` or
/// `json`). Returns `Err` only for an unknown format.
pub fn run(
    email: &str,
    check_disposable: bool,
    check_role: bool,
    format: &str,
) -> Result<String, String> {
    let fmt = match format.trim().to_ascii_lowercase().as_str() {
        "" | "report" => "report",
        "json" => "json",
        "summary" => "summary",
        other => {
            return Err(format!(
                "invalid format {other:?}: expected 'report', 'summary', or 'json'"
            ))
        }
    };

    let v = validate(email);
    let detection = if check_disposable {
        Some(detect(&v.normalized))
    } else {
        None
    };
    let disposable = detection.as_ref().map(|d| d.disposable).unwrap_or(false);
    let role_based = check_role && v.local.as_deref().map(role_local).unwrap_or(false);

    // Role addresses are deliverable, so they warn rather than fail; only bad
    // syntax or a throwaway domain is a hard fail.
    let verdict = if v.valid && !disposable { "pass" } else { "fail" };
    // Risk grades what the verdict cannot: `medium` marks a passing address
    // that still needs a human look (a role alias, or a likely domain typo).
    // Cosmetic warnings such as "had surrounding whitespace" do not raise it.
    let risk = if !v.valid || disposable {
        "high"
    } else if role_based || v.suggestion.is_some() {
        "medium"
    } else {
        "low"
    };

    match fmt {
        "summary" => Ok(format!(
            "{verdict}: {} (syntax: {}, disposable: {}, role_based: {}, risk: {risk})",
            v.normalized,
            if v.valid { "valid" } else { "invalid" },
            yes_no(disposable),
            yes_no(role_based)
        )),
        "json" => Ok(format!(
            "{{\"email\":{:?},\"valid\":{},\"disposable\":{},\"role_based\":{},\"risk\":{:?},\"verdict\":{:?},\"errors\":{},\"warnings\":{},\"suggestion\":{}}}",
            v.normalized,
            v.valid,
            disposable,
            role_based,
            risk,
            verdict,
            json_array(&v.errors),
            json_array(&v.warnings),
            json_opt(v.suggestion.as_deref()),
        )),
        _ => {
            let mut out = String::new();
            out.push_str(&format!("Email: {}\n", v.normalized));
            out.push_str(&format!(
                "Syntax: {}\n",
                if v.valid { "valid" } else { "invalid" }
            ));
            if let Some(local) = &v.local {
                out.push_str(&format!("Local part: {local}\n"));
            }
            if let Some(domain) = &v.domain {
                out.push_str(&format!("Domain: {domain}\n"));
            }
            out.push_str(&format!(
                "Disposable: {}\n",
                match &detection {
                    Some(d) => yes_no(d.disposable),
                    None => "not checked",
                }
            ));
            out.push_str(&format!(
                "Role-based: {}\n",
                if check_role {
                    yes_no(role_based)
                } else {
                    "not checked"
                }
            ));
            out.push_str(&format!("Risk: {risk}\n"));
            out.push_str(&format!("Verdict: {verdict}\n"));
            if !v.errors.is_empty() {
                out.push_str("Errors:\n");
                for e in &v.errors {
                    out.push_str(&format!("- {e}\n"));
                }
            }
            if !v.warnings.is_empty() {
                out.push_str("Warnings:\n");
                for w in &v.warnings {
                    out.push_str(&format!("- {w}\n"));
                }
            }
            if let Some(s) = &v.suggestion {
                out.push_str(&format!("Suggestion: {s}\n"));
            }
            if let Some(d) = &detection {
                out.push_str(&format!("Disposable reason: {}\n", d.reason));
                if let Some(matched) = &d.matched_domain {
                    out.push_str(&format!("Matched disposable domain: {matched}\n"));
                }
            }
            if role_based {
                out.push_str(
                    "Role note: this local part is a shared team or automated alias, which many signup and marketing workflows review or block.\n",
                );
            }
            out.push_str(
                "Checks are offline: no DNS, MX or SMTP lookup was performed, so a pass does not prove the mailbox exists.\n",
            );
            Ok(out.trim_end().to_string())
        }
    }
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

/// Render a string list as a JSON array (`{:?}` on a `Vec<String>` is Rust
/// debug syntax, which is *not* valid JSON for every input).
fn json_array(items: &[String]) -> String {
    let inner: Vec<String> = items.iter().map(|s| json_string(s)).collect();
    format!("[{}]", inner.join(","))
}

fn json_opt(item: Option<&str>) -> String {
    match item {
        Some(s) => json_string(s),
        None => "null".to_string(),
    }
}

/// Minimal JSON string escaper (the crate is dependency-free by design).
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_business_email_passes() {
        let out = run(" Ada <ada@example.com> ", true, true, "report").unwrap();
        assert!(out.contains("Email: ada@example.com"), "{out}");
        assert!(out.contains("Syntax: valid"), "{out}");
        assert!(out.contains("Local part: ada"), "{out}");
        assert!(out.contains("Domain: example.com"), "{out}");
        assert!(out.contains("Disposable: no"), "{out}");
        assert!(out.contains("Role-based: no"), "{out}");
        assert!(out.contains("Risk: low"), "{out}");
        assert!(out.contains("Verdict: pass"), "{out}");
    }

    #[test]
    fn flags_disposable_and_role_based() {
        let out = run("admin@mailinator.com", true, true, "summary").unwrap();
        assert_eq!(
            out,
            "fail: admin@mailinator.com (syntax: valid, disposable: yes, role_based: yes, risk: high)"
        );
    }

    #[test]
    fn role_address_on_a_normal_domain_is_medium_risk_but_still_passes() {
        let out = run("Info@Example.com", true, true, "summary").unwrap();
        assert_eq!(
            out,
            "pass: Info@Example.com (syntax: valid, disposable: no, role_based: yes, risk: medium)"
        );
    }

    #[test]
    fn underscore_role_alias_is_normalized() {
        assert!(run("no_reply@example.com", false, true, "summary")
            .unwrap()
            .contains("role_based: yes"));
    }

    #[test]
    fn disabled_checks_are_reported_as_not_checked() {
        let out = run("admin@mailinator.com", false, false, "report").unwrap();
        assert!(out.contains("Disposable: not checked"), "{out}");
        assert!(out.contains("Role-based: not checked"), "{out}");
        assert!(out.contains("Verdict: pass"), "{out}");
        assert!(!out.contains("Disposable reason:"), "{out}");
    }

    #[test]
    fn json_is_machine_readable() {
        let out = run("ada@example.com", true, true, "json").unwrap();
        assert_eq!(
            out,
            "{\"email\":\"ada@example.com\",\"valid\":true,\"disposable\":false,\"role_based\":false,\"risk\":\"low\",\"verdict\":\"pass\",\"errors\":[],\"warnings\":[],\"suggestion\":null}"
        );
    }

    #[test]
    fn typo_suggestion_is_surfaced() {
        let out = run("ada@gmial.com", true, true, "report").unwrap();
        assert!(out.contains("Suggestion: ada@gmail.com"), "{out}");
        assert!(out.contains("Risk: medium"), "{out}");
    }

    #[test]
    fn reports_syntax_errors() {
        let out = run("not an address", true, true, "report").unwrap();
        assert!(out.contains("missing '@'"), "{out}");
        assert!(out.contains("Syntax: invalid"), "{out}");
        assert!(out.contains("Risk: high"), "{out}");
        assert!(out.contains("Verdict: fail"), "{out}");
    }

    #[test]
    fn rejects_unknown_format() {
        let err = run("a@example.com", true, true, "xml").unwrap_err();
        assert!(err.contains("invalid format"), "{err}");
        assert!(err.contains("'report', 'summary', or 'json'"), "{err}");
    }

    #[test]
    fn role_list_is_populated() {
        assert!(role_count() >= 30);
    }
}
