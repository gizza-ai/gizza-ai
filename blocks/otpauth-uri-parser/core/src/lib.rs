//! otpauth-uri-parser core — parse a single `otpauth://` provisioning URI (the Key Uri
//! Format used by authenticator apps) into its fields: type, issuer, account, base32
//! secret, algorithm, digits, and period (TOTP) or counter (HOTP). Pure Rust, no I/O;
//! percent-decoding and base32 validation are hand-rolled so the crate stays tiny and
//! instantiates on every backend.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Key Uri Format default when `algorithm` is absent.
pub const DEFAULT_ALGORITHM: &str = "SHA1";
/// Key Uri Format default when `digits` is absent.
pub const DEFAULT_DIGITS: u32 = 6;
/// Key Uri Format default when `period` is absent (TOTP only).
pub const DEFAULT_PERIOD: u64 = 30;
/// Longest URI accepted, in characters. Real provisioning URIs are well under 300.
pub const MAX_URI_CHARS: usize = 4096;

/// The parameter names the Key Uri Format defines. Anything else is reported as extra.
const KNOWN_PARAMS: [&str; 6] = ["secret", "issuer", "algorithm", "digits", "period", "counter"];

/// Every field recovered from an `otpauth://` URI, with defaults already applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    /// `totp` or `hotp`, lower-cased.
    pub otp_type: String,
    /// The full label between the type and the query string, percent-decoded.
    pub label: String,
    /// The issuer actually in effect (query parameter wins over the label prefix).
    pub issuer: String,
    /// The issuer prefix taken from the label, if the label carried one.
    pub issuer_in_label: Option<String>,
    /// The value of the `issuer` query parameter, if present.
    pub issuer_in_query: Option<String>,
    /// The account name — the part of the label after the issuer prefix.
    pub account: String,
    /// The shared secret, normalized to unpadded upper-case base32.
    pub secret: String,
    /// Length of `secret` in base32 characters.
    pub secret_chars: usize,
    /// Length of the secret once base32-decoded, in bytes.
    pub secret_bytes: usize,
    /// `SHA1`, `SHA256`, or `SHA512`.
    pub algorithm: String,
    /// Number of digits in the generated code.
    pub digits: u32,
    /// TOTP time step in seconds; `None` for HOTP.
    pub period: Option<u64>,
    /// HOTP counter; `None` for TOTP.
    pub counter: Option<u64>,
    /// Query parameters outside the Key Uri Format, kept verbatim.
    pub extra_parameters: BTreeMap<String, String>,
    /// Field names that were absent from the URI and filled in from the spec defaults.
    pub defaults_applied: Vec<String>,
    /// Non-fatal problems: issuer mismatch, odd digit counts, ignored parameters, …
    pub warnings: Vec<String>,
}

/// How the parsed fields are rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Text,
    Table,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "json" | "" => Ok(Format::Json),
            "text" => Ok(Format::Text),
            "table" => Ok(Format::Table),
            other => Err(format!("unknown format '{other}' (use json, text, or table)")),
        }
    }
}

/// Percent-decode `s` per RFC 3986. A `%` not followed by two hex digits is kept
/// literally (a lone `%` in a hand-written label should not fail the whole parse).
/// `+` is NOT treated as a space — the Key Uri Format is RFC 3986, not form encoding.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hi = (b[i + 1] as char).to_digit(16);
            let lo = (b[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Position + width of the first label separator: a literal `:` or a percent-encoded
/// `%3A` (either case), whichever comes first.
fn find_label_colon(raw: &str) -> Option<(usize, usize)> {
    let literal = raw.find(':');
    let encoded = raw.to_ascii_lowercase().find("%3a");
    match (literal, encoded) {
        (Some(a), Some(b)) => Some(if a <= b { (a, 1) } else { (b, 3) }),
        (Some(a), None) => Some((a, 1)),
        (None, Some(b)) => Some((b, 3)),
        (None, None) => None,
    }
}

/// Number of bytes a base32 string of `chars` characters decodes to (RFC 4648, no padding).
fn base32_decoded_len(chars: usize) -> usize {
    chars * 5 / 8
}

/// Normalize a base32 secret: drop spaces, hyphens, underscores and `=` padding, then
/// upper-case. Returns the normalized secret plus whether anything actually changed.
fn normalize_secret(raw: &str) -> (String, bool) {
    let normalized: String = raw
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-' && *c != '_' && *c != '=')
        .flat_map(|c| c.to_uppercase())
        .collect();
    (normalized.clone(), normalized != raw)
}

/// Validate a normalized base32 secret against the RFC 4648 alphabet + length rules.
fn validate_secret(secret: &str) -> Result<(), String> {
    if secret.is_empty() {
        return Err("the secret parameter is empty; otpauth URIs must carry a base32 secret".into());
    }
    if let Some(bad) = secret.chars().find(|c| !matches!(c, 'A'..='Z' | '2'..='7')) {
        return Err(format!(
            "secret contains '{bad}', which is not a base32 character (expected A-Z and 2-7 only, per RFC 4648)"
        ));
    }
    // A base32 group is 8 characters; only these remainders can come from whole bytes.
    if !matches!(secret.len() % 8, 0 | 2 | 4 | 5 | 7) {
        return Err(format!(
            "secret has an invalid base32 length ({} characters); the final group must hold 2, 4, 5, 7 or 8 characters",
            secret.len()
        ));
    }
    Ok(())
}

/// Normalize an algorithm name (`sha-256`, `Sha256`, `SHA_256` all mean SHA256).
fn parse_algorithm(raw: &str) -> Result<String, String> {
    match raw.trim().to_ascii_lowercase().replace(['-', '_'], "").as_str() {
        "sha1" => Ok("SHA1".to_string()),
        "sha256" => Ok("SHA256".to_string()),
        "sha512" => Ok("SHA512".to_string()),
        other => Err(format!(
            "algorithm '{other}' is not in the Key Uri Format (expected SHA1, SHA256, or SHA512)"
        )),
    }
}

/// Parse an `otpauth://` provisioning URI into its fields.
///
/// `strict` turns the spec's "should" rules into hard errors: a missing issuer, a digit
/// count other than 6 or 8, an issuer mismatch between label and query, a duplicate
/// parameter, and any parameter outside the Key Uri Format all fail instead of warning.
pub fn parse(uri: &str, strict: bool) -> Result<Parsed, String> {
    let mut warnings: Vec<String> = Vec::new();

    let trimmed = uri.trim();
    if trimmed.is_empty() {
        return Err("no URI given: paste an otpauth:// provisioning URI, e.g. otpauth://totp/ACME:alice@example.com?secret=JBSWY3DPEHPK3PXP&issuer=ACME".into());
    }
    if trimmed.chars().count() > MAX_URI_CHARS {
        return Err(format!(
            "URI is too long ({} characters); the limit is {MAX_URI_CHARS}",
            trimmed.chars().count()
        ));
    }

    // A URI pasted out of an email or a terminal is often wrapped across lines. Line
    // breaks and tabs can never appear inside a real URI, so drop them (spaces stay:
    // some issuers emit an un-encoded space in the label).
    let raw: String = trimmed.chars().filter(|c| !matches!(c, '\n' | '\r' | '\t')).collect();
    if raw.chars().count() != trimmed.chars().count() {
        warnings.push("removed line breaks from the pasted URI before parsing".into());
    }
    // Drop a fragment — nothing in the Key Uri Format lives after '#'.
    let raw = match raw.find('#') {
        Some(i) => raw[..i].to_string(),
        None => raw,
    };

    let lower = raw.to_ascii_lowercase();
    if lower.starts_with("otpauth-migration://") {
        return Err("this is a Google Authenticator migration export (otpauth-migration://offline?data=...), which bundles several accounts in one payload — decode the export first, then parse one of the otpauth:// URIs it produces".into());
    }
    if !lower.starts_with("otpauth://") {
        let head: String = raw.chars().take(40).collect();
        return Err(format!(
            "expected a URI starting with otpauth://, got '{head}'"
        ));
    }

    let rest = &raw["otpauth://".len()..];
    let (path, query) = match rest.find('?') {
        Some(i) => (&rest[..i], &rest[i + 1..]),
        None => (rest, ""),
    };

    let (type_raw, label_raw) = match path.find('/') {
        Some(i) => (&path[..i], &path[i + 1..]),
        None => (path, ""),
    };
    let otp_type = type_raw.trim().to_ascii_lowercase();
    if otp_type != "totp" && otp_type != "hotp" {
        let shown = if otp_type.is_empty() { "(empty)".to_string() } else { otp_type.clone() };
        return Err(format!(
            "unknown OTP type '{shown}' (expected totp for time-based or hotp for counter-based codes)"
        ));
    }

    // --- label → issuer prefix + account -------------------------------------------
    let label = percent_decode(label_raw);
    let (issuer_in_label, account) = match find_label_colon(label_raw) {
        Some((idx, width)) => {
            let prefix = percent_decode(&label_raw[..idx]);
            let tail = percent_decode(&label_raw[idx + width..]);
            // The spec allows optional spaces (literal or %20) after the separator.
            let account = tail.trim_start().to_string();
            if account.contains(':') {
                warnings.push(
                    "the label holds more than one colon; everything after the FIRST colon was read as the account name".into(),
                );
            }
            let prefix = prefix.trim().to_string();
            if prefix.is_empty() {
                warnings.push("the label starts with a colon but has no issuer before it".into());
                (None, account)
            } else {
                (Some(prefix), account)
            }
        }
        None => (None, label.clone()),
    };
    if account.is_empty() {
        warnings.push(
            "the label has no account name; most authenticator apps show a blank entry for this".into(),
        );
    }

    // --- query parameters ------------------------------------------------------------
    let mut params: BTreeMap<String, String> = BTreeMap::new();
    let mut extra_parameters: BTreeMap<String, String> = BTreeMap::new();
    let mut duplicates: Vec<String> = Vec::new();
    let mut unknown: Vec<String> = Vec::new();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (k_raw, v_raw) = match pair.find('=') {
            Some(i) => (&pair[..i], &pair[i + 1..]),
            None => (pair, ""),
        };
        let key = percent_decode(k_raw).trim().to_ascii_lowercase();
        let value = percent_decode(v_raw);
        if key.is_empty() {
            continue;
        }
        let known = KNOWN_PARAMS.contains(&key.as_str());
        let slot = if known { &mut params } else { &mut extra_parameters };
        if slot.contains_key(&key) {
            duplicates.push(key.clone());
            continue; // first value wins
        }
        if !known {
            unknown.push(key.clone());
        }
        slot.insert(key, value);
    }
    for key in &duplicates {
        let msg = format!("parameter '{key}' appears more than once; the first value was used");
        if strict {
            return Err(msg);
        }
        warnings.push(msg);
    }
    for key in &unknown {
        let msg = format!(
            "parameter '{key}' is not part of the Key Uri Format; it is reported under extra_parameters and ignored by most apps"
        );
        if strict {
            return Err(msg);
        }
        warnings.push(msg);
    }

    let mut defaults_applied: Vec<String> = Vec::new();

    // --- secret ------------------------------------------------------------------------
    let secret_raw = params.get("secret").ok_or_else(|| {
        "missing the required secret parameter (otpauth://<type>/<label>?secret=<base32>)".to_string()
    })?;
    let (secret, changed) = normalize_secret(secret_raw);
    validate_secret(&secret)?;
    if changed {
        warnings.push(
            "the secret was normalized: spaces, hyphens and '=' padding removed and letters upper-cased".into(),
        );
    }
    let secret_chars = secret.len();
    let secret_bytes = base32_decoded_len(secret_chars);

    // --- issuer ---------------------------------------------------------------------------
    let issuer_in_query = params.get("issuer").map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    if let (Some(l), Some(q)) = (&issuer_in_label, &issuer_in_query) {
        if l != q {
            let msg = format!(
                "issuer mismatch: the label says '{l}' but the issuer parameter says '{q}'; the Key Uri Format requires them to match, and apps disagree on which one wins"
            );
            if strict {
                return Err(msg);
            }
            warnings.push(msg);
        }
    }
    let issuer = issuer_in_query.clone().or_else(|| issuer_in_label.clone()).unwrap_or_default();
    if issuer.is_empty() {
        let msg = "no issuer given in either the label prefix or the issuer parameter; the spec strongly recommends both".to_string();
        if strict {
            return Err(msg);
        }
        warnings.push(msg);
    } else if issuer_in_label.is_none() {
        warnings.push(
            "the issuer is only in the issuer parameter; adding it as a label prefix too ('Issuer:account') improves app compatibility".into(),
        );
    } else if issuer_in_query.is_none() {
        warnings.push(
            "the issuer is only in the label prefix; adding an issuer= parameter too improves app compatibility".into(),
        );
    }

    // --- algorithm ---------------------------------------------------------------------------
    let algorithm = match params.get("algorithm") {
        Some(v) => parse_algorithm(v)?,
        None => {
            defaults_applied.push("algorithm".into());
            DEFAULT_ALGORITHM.to_string()
        }
    };

    // --- digits -------------------------------------------------------------------------------
    let digits = match params.get("digits") {
        Some(v) => {
            let n: u32 = v
                .trim()
                .parse()
                .map_err(|_| format!("digits must be a whole number, got '{v}'"))?;
            if n != 6 && n != 8 {
                let msg = format!(
                    "digits is {n}; the Key Uri Format only defines 6 and 8, so many authenticator apps will fall back to 6"
                );
                if strict {
                    return Err(msg);
                }
                if !(1..=10).contains(&n) {
                    return Err(format!("digits must be between 1 and 10, got {n}"));
                }
                warnings.push(msg);
            }
            n
        }
        None => {
            defaults_applied.push("digits".into());
            DEFAULT_DIGITS
        }
    };

    // --- period / counter -------------------------------------------------------------------
    let mut period = None;
    let mut counter = None;
    if otp_type == "totp" {
        period = Some(match params.get("period") {
            Some(v) => {
                let n: u64 = v
                    .trim()
                    .parse()
                    .map_err(|_| format!("period must be a whole number of seconds, got '{v}'"))?;
                if n == 0 {
                    return Err("period must be at least 1 second, got 0".into());
                }
                n
            }
            None => {
                defaults_applied.push("period".into());
                DEFAULT_PERIOD
            }
        });
        if params.contains_key("counter") {
            warnings.push(
                "counter is set on a totp URI; counters only apply to hotp and this value is ignored".into(),
            );
        }
    } else {
        counter = Some(match params.get("counter") {
            Some(v) => v
                .trim()
                .parse()
                .map_err(|_| format!("counter must be a whole number, got '{v}'"))?,
            None => {
                return Err(
                    "hotp URIs must carry a counter parameter (the initial counter value)".into()
                )
            }
        });
        if params.contains_key("period") {
            warnings.push(
                "period is set on an hotp URI; periods only apply to totp and this value is ignored".into(),
            );
        }
    }

    Ok(Parsed {
        otp_type,
        label,
        issuer,
        issuer_in_label,
        issuer_in_query,
        account,
        secret,
        secret_chars,
        secret_bytes,
        algorithm,
        digits,
        period,
        counter,
        extra_parameters,
        defaults_applied,
        warnings,
    })
}

/// Mask a secret for sharing: keep nothing but the length.
fn masked_secret(secret: &str) -> String {
    "*".repeat(secret.len().min(8))
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

fn json_string(s: &str) -> String {
    format!("\"{}\"", json_escape(s))
}

fn json_opt_string(v: &Option<String>) -> String {
    match v {
        Some(s) => json_string(s),
        None => "null".to_string(),
    }
}

fn json_array(items: &[String]) -> String {
    if items.is_empty() {
        return "[]".to_string();
    }
    let body: Vec<String> = items.iter().map(|s| format!("    {}", json_string(s))).collect();
    format!("[\n{}\n  ]", body.join(",\n"))
}

fn json_object(map: &BTreeMap<String, String>) -> String {
    if map.is_empty() {
        return "{}".to_string();
    }
    let body: Vec<String> = map
        .iter()
        .map(|(k, v)| format!("    {}: {}", json_string(k), json_string(v)))
        .collect();
    format!("{{\n{}\n  }}", body.join(",\n"))
}

/// Render the parsed fields as pretty-printed JSON.
pub fn to_json(p: &Parsed, mask: bool) -> String {
    let secret = if mask { masked_secret(&p.secret) } else { p.secret.clone() };
    let mut out = String::new();
    out.push_str("{\n");
    let _ = writeln!(out, "  \"type\": {},", json_string(&p.otp_type));
    let _ = writeln!(out, "  \"label\": {},", json_string(&p.label));
    let _ = writeln!(out, "  \"issuer\": {},", json_string(&p.issuer));
    let _ = writeln!(out, "  \"issuer_in_label\": {},", json_opt_string(&p.issuer_in_label));
    let _ = writeln!(out, "  \"issuer_in_query\": {},", json_opt_string(&p.issuer_in_query));
    let _ = writeln!(out, "  \"account\": {},", json_string(&p.account));
    let _ = writeln!(out, "  \"secret\": {},", json_string(&secret));
    let _ = writeln!(out, "  \"secret_masked\": {},", mask);
    let _ = writeln!(out, "  \"secret_chars\": {},", p.secret_chars);
    let _ = writeln!(out, "  \"secret_bytes\": {},", p.secret_bytes);
    let _ = writeln!(out, "  \"algorithm\": {},", json_string(&p.algorithm));
    let _ = writeln!(out, "  \"digits\": {},", p.digits);
    match p.period {
        Some(n) => {
            let _ = writeln!(out, "  \"period\": {n},");
        }
        None => {
            let _ = writeln!(out, "  \"period\": null,");
        }
    }
    match p.counter {
        Some(n) => {
            let _ = writeln!(out, "  \"counter\": {n},");
        }
        None => {
            let _ = writeln!(out, "  \"counter\": null,");
        }
    }
    let _ = writeln!(out, "  \"extra_parameters\": {},", json_object(&p.extra_parameters));
    let _ = writeln!(out, "  \"defaults_applied\": {},", json_array(&p.defaults_applied));
    let _ = writeln!(out, "  \"warnings\": {}", json_array(&p.warnings));
    out.push('}');
    out
}

/// The label/value rows shared by the text and table renderers.
fn rows(p: &Parsed, mask: bool) -> Vec<(String, String)> {
    let secret = if mask { masked_secret(&p.secret) } else { p.secret.clone() };
    let defaulted = |name: &str| {
        if p.defaults_applied.iter().any(|d| d == name) {
            " (default)"
        } else {
            ""
        }
    };
    let mut rows = vec![
        ("type".to_string(), p.otp_type.clone()),
        (
            "issuer".to_string(),
            if p.issuer.is_empty() { "(none)".to_string() } else { p.issuer.clone() },
        ),
        (
            "account".to_string(),
            if p.account.is_empty() { "(none)".to_string() } else { p.account.clone() },
        ),
        ("label".to_string(), p.label.clone()),
        ("secret".to_string(), secret),
        ("secret size".to_string(), format!("{} chars, {} bytes", p.secret_chars, p.secret_bytes)),
        ("algorithm".to_string(), format!("{}{}", p.algorithm, defaulted("algorithm"))),
        ("digits".to_string(), format!("{}{}", p.digits, defaulted("digits"))),
    ];
    if let Some(n) = p.period {
        rows.push(("period".to_string(), format!("{n} seconds{}", defaulted("period"))));
    }
    if let Some(n) = p.counter {
        rows.push(("counter".to_string(), n.to_string()));
    }
    for (k, v) in &p.extra_parameters {
        rows.push((format!("extra: {k}"), v.clone()));
    }
    rows
}

/// Render the parsed fields as aligned `key: value` lines.
pub fn to_text(p: &Parsed, mask: bool) -> String {
    let rows = rows(p, mask);
    let width = rows.iter().map(|(k, _)| k.chars().count()).max().unwrap_or(0);
    let mut out = String::new();
    for (k, v) in &rows {
        let _ = writeln!(out, "{k}:{} {v}", " ".repeat(width - k.chars().count()));
    }
    if !p.warnings.is_empty() {
        out.push('\n');
        let _ = writeln!(out, "Warnings ({}):", p.warnings.len());
        for w in &p.warnings {
            let _ = writeln!(out, "  - {w}");
        }
    }
    out.trim_end().to_string()
}

/// Render the parsed fields as an ASCII table.
pub fn to_table(p: &Parsed, mask: bool) -> String {
    let rows = rows(p, mask);
    let head = ("Field", "Value");
    let kw = rows
        .iter()
        .map(|(k, _)| k.chars().count())
        .chain(std::iter::once(head.0.len()))
        .max()
        .unwrap_or(5);
    let vw = rows
        .iter()
        .map(|(_, v)| v.chars().count())
        .chain(std::iter::once(head.1.len()))
        .max()
        .unwrap_or(5);
    let rule = format!("+-{}-+-{}-+", "-".repeat(kw), "-".repeat(vw));
    let pad = |s: &str, w: usize| format!("{s}{}", " ".repeat(w - s.chars().count()));

    let mut out = String::new();
    let _ = writeln!(out, "{rule}");
    let _ = writeln!(out, "| {} | {} |", pad(head.0, kw), pad(head.1, vw));
    let _ = writeln!(out, "{rule}");
    for (k, v) in &rows {
        let _ = writeln!(out, "| {} | {} |", pad(k, kw), pad(v, vw));
    }
    let _ = writeln!(out, "{rule}");
    if !p.warnings.is_empty() {
        out.push('\n');
        let _ = writeln!(out, "Warnings ({}):", p.warnings.len());
        for w in &p.warnings {
            let _ = writeln!(out, "  - {w}");
        }
    }
    out.trim_end().to_string()
}

/// Parse `uri` and render it in `format`. `mask_secret` replaces the secret with
/// asterisks (the reported lengths stay accurate) so the result is safe to paste into a
/// ticket; `strict` fails on the spec's "should" rules instead of warning.
pub fn run_with_options(
    uri: &str,
    format: &str,
    mask_secret: bool,
    strict: bool,
) -> Result<String, String> {
    let format = Format::parse(format)?;
    let parsed = parse(uri, strict)?;
    Ok(match format {
        Format::Json => to_json(&parsed, mask_secret),
        Format::Text => to_text(&parsed, mask_secret),
        Format::Table => to_table(&parsed, mask_secret),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOTP: &str = "otpauth://totp/ACME%20Co:john.doe@email.com?secret=HXDMVJECJJWSRB3HWIZR4IFUGFTMXBOZ&issuer=ACME%20Co&algorithm=SHA1&digits=6&period=30";

    #[test]
    fn parses_a_full_totp_uri() {
        let p = parse(TOTP, false).unwrap();
        assert_eq!(p.otp_type, "totp");
        assert_eq!(p.issuer, "ACME Co");
        assert_eq!(p.account, "john.doe@email.com");
        assert_eq!(p.label, "ACME Co:john.doe@email.com");
        assert_eq!(p.secret, "HXDMVJECJJWSRB3HWIZR4IFUGFTMXBOZ");
        assert_eq!(p.secret_chars, 32);
        assert_eq!(p.secret_bytes, 20);
        assert_eq!(p.algorithm, "SHA1");
        assert_eq!(p.digits, 6);
        assert_eq!(p.period, Some(30));
        assert_eq!(p.counter, None);
        assert!(p.defaults_applied.is_empty());
        assert!(p.warnings.is_empty(), "unexpected warnings: {:?}", p.warnings);
    }

    #[test]
    fn applies_key_uri_format_defaults() {
        let p = parse("otpauth://totp/Example:alice@example.com?secret=JBSWY3DPEHPK3PXP&issuer=Example", false)
            .unwrap();
        assert_eq!(p.algorithm, "SHA1");
        assert_eq!(p.digits, 6);
        assert_eq!(p.period, Some(30));
        assert_eq!(p.defaults_applied, vec!["algorithm", "digits", "period"]);
    }

    #[test]
    fn decodes_percent_encoded_label_and_issuer() {
        let p = parse(
            "otpauth://totp/Big%20Corp%3Aalice%40example.com?secret=JBSWY3DPEHPK3PXP&issuer=Big%20Corp",
            false,
        )
        .unwrap();
        assert_eq!(p.issuer, "Big Corp");
        assert_eq!(p.account, "alice@example.com");
        assert_eq!(p.issuer_in_label.as_deref(), Some("Big Corp"));
        assert_eq!(p.issuer_in_query.as_deref(), Some("Big Corp"));
    }

    #[test]
    fn skips_optional_spaces_after_the_label_colon() {
        let p = parse("otpauth://totp/Example:%20alice@example.com?secret=JBSWY3DPEHPK3PXP&issuer=Example", false)
            .unwrap();
        assert_eq!(p.account, "alice@example.com");
    }

    #[test]
    fn parses_an_hotp_uri_with_a_counter() {
        let p = parse("otpauth://hotp/ACME:bob?secret=K5XXE3DE&issuer=ACME&counter=5&digits=8", false).unwrap();
        assert_eq!(p.otp_type, "hotp");
        assert_eq!(p.counter, Some(5));
        assert_eq!(p.period, None);
        assert_eq!(p.digits, 8);
    }

    #[test]
    fn hotp_without_a_counter_is_an_error() {
        let e = parse("otpauth://hotp/ACME:bob?secret=K5XXE3DE&issuer=ACME", false).unwrap_err();
        assert!(e.contains("counter"), "{e}");
    }

    #[test]
    fn warns_when_the_label_issuer_differs_from_the_parameter() {
        let p = parse("otpauth://totp/GitHub:alice?secret=JBSWY3DPEHPK3PXP&issuer=GitLab", false).unwrap();
        assert_eq!(p.issuer, "GitLab");
        assert!(
            p.warnings.iter().any(|w| w.contains("issuer mismatch")),
            "{:?}",
            p.warnings
        );
    }

    #[test]
    fn strict_mode_rejects_an_issuer_mismatch() {
        let e = parse("otpauth://totp/GitHub:alice?secret=JBSWY3DPEHPK3PXP&issuer=GitLab", true).unwrap_err();
        assert!(e.contains("issuer mismatch"), "{e}");
    }

    #[test]
    fn strict_mode_rejects_a_missing_issuer() {
        let e = parse("otpauth://totp/alice?secret=JBSWY3DPEHPK3PXP", true).unwrap_err();
        assert!(e.contains("no issuer"), "{e}");
    }

    #[test]
    fn normalizes_a_spaced_lower_case_padded_secret() {
        let p = parse(
            "otpauth://totp/Example:alice?secret=jbsw%20y3dp%20ehpk%203pxp%3D%3D&issuer=Example",
            false,
        )
        .unwrap();
        assert_eq!(p.secret, "JBSWY3DPEHPK3PXP");
        assert!(p.warnings.iter().any(|w| w.contains("normalized")), "{:?}", p.warnings);
    }

    #[test]
    fn rejects_a_non_base32_secret() {
        let e = parse("otpauth://totp/Example:alice?secret=NOT!BASE32&issuer=Example", false).unwrap_err();
        assert!(e.contains("base32"), "{e}");
    }

    #[test]
    fn rejects_a_base32_secret_of_impossible_length() {
        let e = parse("otpauth://totp/Example:alice?secret=ABC&issuer=Example", false).unwrap_err();
        assert!(e.contains("invalid base32 length"), "{e}");
    }

    #[test]
    fn rejects_a_missing_secret() {
        let e = parse("otpauth://totp/Example:alice?issuer=Example", false).unwrap_err();
        assert!(e.contains("secret"), "{e}");
    }

    #[test]
    fn rejects_a_non_otpauth_uri() {
        let e = parse("https://example.com/totp", false).unwrap_err();
        assert!(e.contains("otpauth://"), "{e}");
    }

    #[test]
    fn points_migration_payloads_at_the_right_tool() {
        let e = parse("otpauth-migration://offline?data=CikKBUhlbGxv", false).unwrap_err();
        assert!(e.contains("migration export"), "{e}");
    }

    #[test]
    fn rejects_an_unknown_otp_type() {
        let e = parse("otpauth://steam/Valve:alice?secret=JBSWY3DPEHPK3PXP", false).unwrap_err();
        assert!(e.contains("totp"), "{e}");
    }

    #[test]
    fn rejects_an_unknown_algorithm() {
        let e = parse(
            "otpauth://totp/Example:alice?secret=JBSWY3DPEHPK3PXP&issuer=Example&algorithm=MD5",
            false,
        )
        .unwrap_err();
        assert!(e.contains("SHA256"), "{e}");
    }

    #[test]
    fn accepts_dashed_algorithm_spellings() {
        let p = parse(
            "otpauth://totp/Example:alice?secret=JBSWY3DPEHPK3PXP&issuer=Example&algorithm=sha-256",
            false,
        )
        .unwrap();
        assert_eq!(p.algorithm, "SHA256");
    }

    #[test]
    fn keeps_unknown_parameters_and_warns() {
        let p = parse(
            "otpauth://totp/Example:alice?secret=JBSWY3DPEHPK3PXP&issuer=Example&image=https%3A%2F%2Fexample.com%2Ficon.png",
            false,
        )
        .unwrap();
        assert_eq!(p.extra_parameters.get("image").map(String::as_str), Some("https://example.com/icon.png"));
        assert!(p.warnings.iter().any(|w| w.contains("not part of the Key Uri Format")));
    }

    #[test]
    fn strict_mode_rejects_unknown_parameters() {
        let e = parse(
            "otpauth://totp/Example:alice?secret=JBSWY3DPEHPK3PXP&issuer=Example&image=x",
            true,
        )
        .unwrap_err();
        assert!(e.contains("not part of the Key Uri Format"), "{e}");
    }

    #[test]
    fn warns_on_a_non_standard_digit_count() {
        let p = parse(
            "otpauth://totp/Example:alice?secret=JBSWY3DPEHPK3PXP&issuer=Example&digits=7",
            false,
        )
        .unwrap();
        assert_eq!(p.digits, 7);
        assert!(p.warnings.iter().any(|w| w.contains("only defines 6 and 8")));
    }

    #[test]
    fn rejects_a_wildly_out_of_range_digit_count() {
        let e = parse(
            "otpauth://totp/Example:alice?secret=JBSWY3DPEHPK3PXP&issuer=Example&digits=99",
            false,
        )
        .unwrap_err();
        assert!(e.contains("between 1 and 10"), "{e}");
    }

    #[test]
    fn rejects_a_zero_period() {
        let e = parse(
            "otpauth://totp/Example:alice?secret=JBSWY3DPEHPK3PXP&issuer=Example&period=0",
            false,
        )
        .unwrap_err();
        assert!(e.contains("at least 1 second"), "{e}");
    }

    #[test]
    fn duplicate_parameters_keep_the_first_value_and_warn() {
        let p = parse(
            "otpauth://totp/Example:alice?secret=JBSWY3DPEHPK3PXP&issuer=Example&digits=8&digits=6",
            false,
        )
        .unwrap();
        assert_eq!(p.digits, 8);
        assert!(p.warnings.iter().any(|w| w.contains("more than once")));
    }

    #[test]
    fn tolerates_a_line_wrapped_paste() {
        let p = parse(
            "otpauth://totp/Example:alice\n?secret=JBSWY3DPEHPK3PXP&issuer=Example",
            false,
        )
        .unwrap();
        assert_eq!(p.account, "alice");
        assert!(p.warnings.iter().any(|w| w.contains("line breaks")));
    }

    #[test]
    fn json_output_is_stable() {
        let out = run_with_options(TOTP, "json", false, false).unwrap();
        assert!(out.starts_with("{\n  \"type\": \"totp\","), "{out}");
        assert!(out.contains("\"secret\": \"HXDMVJECJJWSRB3HWIZR4IFUGFTMXBOZ\""));
        assert!(out.contains("\"secret_bytes\": 20"));
        assert!(out.contains("\"warnings\": []"));
        assert!(out.ends_with('}'));
    }

    #[test]
    fn text_output_lists_every_field() {
        let out = run_with_options(TOTP, "text", false, false).unwrap();
        assert!(out.contains("type:        totp"), "{out}");
        assert!(out.contains("issuer:      ACME Co"), "{out}");
        assert!(out.contains("period:      30 seconds"), "{out}");
    }

    #[test]
    fn table_output_is_boxed() {
        let out = run_with_options(TOTP, "table", false, false).unwrap();
        assert!(out.starts_with('+'), "{out}");
        assert!(out.contains("| Field"), "{out}");
        assert!(out.contains("| account"), "{out}");
    }

    #[test]
    fn masking_hides_the_secret_but_keeps_the_sizes() {
        let out = run_with_options(TOTP, "json", true, false).unwrap();
        assert!(out.contains("\"secret\": \"********\""), "{out}");
        assert!(out.contains("\"secret_masked\": true"));
        assert!(out.contains("\"secret_chars\": 32"));
        assert!(!out.contains("HXDMVJEC"));
    }

    #[test]
    fn rejects_an_unknown_format() {
        let e = run_with_options(TOTP, "yaml", false, false).unwrap_err();
        assert!(e.contains("json, text, or table"), "{e}");
    }

    #[test]
    fn rejects_an_empty_uri() {
        let e = parse("   ", false).unwrap_err();
        assert!(e.contains("no URI given"), "{e}");
    }

    #[test]
    fn rejects_an_over_long_uri() {
        let long = format!("otpauth://totp/{}?secret=JBSWY3DPEHPK3PXP", "a".repeat(MAX_URI_CHARS));
        let e = parse(&long, false).unwrap_err();
        assert!(e.contains("too long"), "{e}");
    }
}
