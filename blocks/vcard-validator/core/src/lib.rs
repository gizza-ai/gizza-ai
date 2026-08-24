//! vcard-validator core — a read-only linter for vCard / `.vcf` documents.
//! Shared by the chat skill block and the web page; no wafer/wasm-bindgen deps.
//!
//! It parses one or many `BEGIN:VCARD … END:VCARD` blocks (vCard 2.1,
//! 3.0 (RFC 2426) and 4.0 (RFC 6350), including folded lines) and reports every
//! problem it finds with a 1-indexed **physical** source line, a severity
//! (`error`/`warning`) and a stable `rule` slug. It NEVER rewrites the document
//! — the sibling `vcard-normalize` tool is the repair surface.
//!
//! Rule groups:
//!   * structure — BEGIN/END pairing, stray content, stray fold lines;
//!   * version   — VERSION present / first / known / matching the expected one;
//!   * required  — FN (3.0/4.0), N (2.1/3.0), plus any user-named properties;
//!   * syntax    — missing ':', property-name charset, parameter forms, folding;
//!   * values    — EMAIL, TEL (via `phonenumber`), dates, URIs, N/ADR arity,
//!                 KIND/GENDER enums;
//!   * hygiene   — duplicate single-instance properties, unknown properties.

use phonenumber::country;
use std::collections::HashMap;

/// The vCard specification version a card is checked against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    /// Use each card's own `VERSION` property (default).
    Auto,
    V21,
    V30,
    V40,
}

impl Version {
    /// Parse the `version` argument. Empty/whitespace → `Auto`.
    pub fn parse(s: &str) -> Result<Version, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Version::Auto),
            "2.1" => Ok(Version::V21),
            "3.0" => Ok(Version::V30),
            "4.0" => Ok(Version::V40),
            other => Err(format!(
                "unknown version '{other}' (use 'auto', '2.1', '3.0', or '4.0')"
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Version::Auto => "auto",
            Version::V21 => "2.1",
            Version::V30 => "3.0",
            Version::V40 => "4.0",
        }
    }
}

/// Output form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// Human-readable grouped report (default).
    Report,
    /// Structured JSON for CI.
    Json,
}

impl Output {
    /// Parse the `output` argument. Empty/whitespace → `Report`.
    pub fn parse(s: &str) -> Result<Output, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "report" => Ok(Output::Report),
            "json" => Ok(Output::Json),
            other => Err(format!("unknown output '{other}' (use 'report' or 'json')")),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Severity {
    Error,
    Warning,
}

impl Severity {
    fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

struct Issue {
    /// 1-indexed physical source line, or 0 for a document-level issue.
    line: usize,
    /// 1-indexed card number, or 0 for a document-level issue.
    card: usize,
    severity: Severity,
    rule: &'static str,
    /// Property name the issue is about, or "" when it isn't property-scoped.
    property: String,
    message: String,
}

/// One logical (unfolded) content line plus the physical line it started on.
struct Logical {
    line: usize,
    text: String,
}

/// Properties that may appear at most once per card in vCard 4.0 (RFC 6350
/// §6, cardinality `*1` or `1`). 2.1/3.0 agree on this subset.
const SINGLE_INSTANCE: &[&str] = &[
    "VERSION",
    "N",
    "BDAY",
    "ANNIVERSARY",
    "GENDER",
    "PRODID",
    "REV",
    "UID",
    "KIND",
];

/// Property names defined by at least one of vCard 2.1 / 3.0 / 4.0. Anything
/// else that is not `X-`-prefixed earns an `unknown-property` warning.
const KNOWN_PROPERTIES: &[&str] = &[
    // Shared across versions
    "ADR", "AGENT", "ANNIVERSARY", "BDAY", "CALADRURI", "CALURI", "CATEGORIES", "CLASS",
    "CLIENTPIDMAP", "EMAIL", "FBURL", "FN", "GENDER", "GEO", "IMPP", "KEY", "KIND", "LABEL",
    "LANG", "LOGO", "MAILER", "MEMBER", "N", "NAME", "NICKNAME", "NOTE", "ORG", "PHOTO",
    "PRODID", "PROFILE", "RELATED", "REV", "ROLE", "SORT-STRING", "SOUND", "SOURCE", "TEL",
    "TITLE", "TZ", "UID", "URL", "VERSION", "XML",
];

/// Validate a vCard document.
///
/// - `data`: the raw vCard / `.vcf` text.
/// - `version`: which spec to check against; `Auto` uses each card's own
///   `VERSION` property (and a card with no `VERSION` is checked as 3.0).
/// - `default_country`: ISO-3166 alpha-2 region hint (e.g. `US`, `GB`) used to
///   interpret `TEL` values written without a `+`. Empty = no hint, in which
///   case a national-format number is reported as unverifiable rather than
///   invalid. An unrecognised code errors.
/// - `check_email` / `check_phone`: enable the `EMAIL` / `TEL` value rules.
/// - `required_properties`: extra comma-separated property names that must be
///   present in every card (e.g. `UID,ORG` for a CardDAV-style profile).
/// - `output`: `Report` (human-readable) or `Json` (structured).
///
/// Returns the report text, or `Err` when the input holds no card at all or an
/// argument is invalid.
pub fn validate(
    data: &str,
    version: Version,
    default_country: &str,
    check_email: bool,
    check_phone: bool,
    required_properties: &str,
    output: Output,
) -> Result<String, String> {
    // Resolve the region hint up front so a bad code fails fast.
    let region = parse_region(default_country)?;
    let required: Vec<String> = required_properties
        .split(',')
        .map(|s| s.trim().to_ascii_uppercase())
        .filter(|s| !s.is_empty())
        .collect();

    let mut issues: Vec<Issue> = Vec::new();
    let cards = split_cards(data, &mut issues)?;

    for (idx, card) in cards.iter().enumerate() {
        check_card(
            idx + 1,
            card,
            version,
            region,
            check_email,
            check_phone,
            &required,
            &mut issues,
        );
    }

    // Document-level: RFC 6350 §3.2 mandates CRLF line breaks.
    if !data.contains("\r\n") && data.lines().count() > 1 {
        issues.push(Issue {
            line: 0,
            card: 0,
            severity: Severity::Warning,
            rule: "lf-line-endings",
            property: String::new(),
            message: "the document uses bare LF line endings; the vCard specification requires CRLF (\\r\\n) — some older address books reject LF-only files".into(),
        });
    }

    issues.sort_by_key(|i| (i.card, i.line));
    Ok(match output {
        Output::Report => render_report(&cards, &issues, version),
        Output::Json => render_json(&cards, &issues),
    })
}

/// A single card: its physical line span, its logical content lines, and the
/// `VERSION` value found inside it (if any).
struct Card {
    begin_line: usize,
    lines: Vec<Logical>,
    version_value: Option<String>,
}

/// Unfold the document and slice it into cards, recording structural issues.
fn split_cards(data: &str, issues: &mut Vec<Issue>) -> Result<Vec<Card>, String> {
    let logical = unfold(data, issues);

    let mut cards: Vec<Card> = Vec::new();
    let mut current: Option<Card> = None;
    let mut card_no = 0usize;

    for l in logical {
        let upper = l.text.trim().to_ascii_uppercase();
        if upper.is_empty() {
            continue;
        }
        if upper == "BEGIN:VCARD" {
            if let Some(open) = current.take() {
                // A nested BEGIN — the previous card never closed.
                card_no += 1;
                issues.push(Issue {
                    line: open.begin_line,
                    card: card_no,
                    severity: Severity::Error,
                    rule: "unclosed-card",
                    property: "BEGIN".into(),
                    message: format!(
                        "the card starting on line {} has no END:VCARD before the next BEGIN:VCARD",
                        open.begin_line
                    ),
                });
                cards.push(open);
            }
            current = Some(Card {
                begin_line: l.line,
                lines: Vec::new(),
                version_value: None,
            });
            continue;
        }
        if upper == "END:VCARD" {
            match current.take() {
                Some(card) => cards.push(card),
                None => issues.push(Issue {
                    line: l.line,
                    card: 0,
                    severity: Severity::Error,
                    rule: "stray-end",
                    property: "END".into(),
                    message: "END:VCARD without a matching BEGIN:VCARD".into(),
                }),
            }
            continue;
        }
        match current.as_mut() {
            Some(card) => card.lines.push(l),
            None => issues.push(Issue {
                line: l.line,
                card: 0,
                severity: Severity::Error,
                rule: "content-outside-card",
                property: String::new(),
                message: format!(
                    "content line outside any BEGIN:VCARD ... END:VCARD block: {}",
                    truncate(l.text.trim(), 60)
                ),
            }),
        }
    }

    if let Some(open) = current.take() {
        cards.push(open);
        let n = cards.len();
        issues.push(Issue {
            line: cards[n - 1].begin_line,
            card: n,
            severity: Severity::Error,
            rule: "unclosed-card",
            property: "BEGIN".into(),
            message: "the document ends without an END:VCARD for this card".into(),
        });
    }

    if cards.is_empty() {
        return Err(
            "no vCard found: expected at least one 'BEGIN:VCARD ... END:VCARD' block".into(),
        );
    }

    // Record each card's VERSION value for auto-detection.
    for card in cards.iter_mut() {
        for l in &card.lines {
            if let Some((head, value)) = split_line(&l.text) {
                if property_name(head).eq_ignore_ascii_case("VERSION") {
                    card.version_value = Some(value.trim().to_string());
                    break;
                }
            }
        }
    }
    Ok(cards)
}

/// Unfold continuation lines (a line beginning with SPACE or TAB continues the
/// previous one) while keeping the PHYSICAL line each logical line started on.
/// Also flags over-long unfolded lines and a fold with nothing to continue.
fn unfold(data: &str, issues: &mut Vec<Issue>) -> Vec<Logical> {
    let mut out: Vec<Logical> = Vec::new();
    for (i, raw) in data.split('\n').enumerate() {
        let line_no = i + 1;
        let line = raw.strip_suffix('\r').unwrap_or(raw);

        // RFC 6350 §3.2: lines SHOULD be folded to at most 75 octets.
        if line.len() > 75 {
            issues.push(Issue {
                line: line_no,
                card: 0,
                severity: Severity::Warning,
                rule: "long-line",
                property: String::new(),
                message: format!(
                    "line is {} octets long; the specification says lines should be folded at 75 octets",
                    line.len()
                ),
            });
        }

        if let Some(rest) = line.strip_prefix(' ').or_else(|| line.strip_prefix('\t')) {
            match out.last_mut() {
                Some(prev) => {
                    prev.text.push_str(rest);
                    continue;
                }
                None => {
                    issues.push(Issue {
                        line: line_no,
                        card: 0,
                        severity: Severity::Error,
                        rule: "stray-fold",
                        property: String::new(),
                        message:
                            "the document starts with a folded continuation line (leading space or tab) with no line to continue"
                                .into(),
                    });
                    continue;
                }
            }
        }
        out.push(Logical {
            line: line_no,
            text: line.to_string(),
        });
    }
    out
}

/// Run every per-card rule.
#[allow(clippy::too_many_arguments)]
fn check_card(
    card_no: usize,
    card: &Card,
    expected: Version,
    region: Option<country::Id>,
    check_email: bool,
    check_phone: bool,
    required: &[String],
    issues: &mut Vec<Issue>,
) {
    // Re-tag the structural issues raised while splitting so they carry a card
    // number: `long-line`/`lf-line-endings` stay document-level by design.
    let effective = effective_version(card, expected, card_no, issues);

    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut seen: Vec<String> = Vec::new();

    for (pos, l) in card.lines.iter().enumerate() {
        let text = l.text.trim_end();
        if text.trim().is_empty() {
            issues.push(Issue {
                line: l.line,
                card: card_no,
                severity: Severity::Warning,
                rule: "blank-line",
                property: String::new(),
                message: "blank line inside a vCard; content lines should be contiguous".into(),
            });
            continue;
        }

        let (head, value) = match split_line(text) {
            Some(hv) => hv,
            None => {
                issues.push(Issue {
                    line: l.line,
                    card: card_no,
                    severity: Severity::Error,
                    rule: "missing-colon",
                    property: String::new(),
                    message: format!(
                        "content line has no ':' separator; expected NAME[;PARAM=VALUE]:VALUE, got {}",
                        truncate(text.trim(), 60)
                    ),
                });
                continue;
            }
        };

        let name_raw = property_name(head);
        let name = name_raw.to_ascii_uppercase();
        check_property_name(card_no, l.line, head, &name, issues);
        check_parameters(card_no, l.line, head, &name, effective, issues);

        if !name.is_empty() {
            *counts.entry(name.clone()).or_insert(0) += 1;
            if !seen.contains(&name) {
                seen.push(name.clone());
            }
        }

        // VERSION must be the first property after BEGIN in vCard 4.0
        // (RFC 6350 §6.7.9) and is conventionally first in 2.1/3.0.
        if name == "VERSION" && pos != 0 {
            let (severity, note) = if effective == Version::V40 {
                (Severity::Error, "must")
            } else {
                (Severity::Warning, "should")
            };
            issues.push(Issue {
                line: l.line,
                card: card_no,
                severity,
                rule: "version-not-first",
                property: "VERSION".into(),
                message: format!(
                    "VERSION {note} be the first property after BEGIN:VCARD in vCard {}, but it is property {} of this card",
                    effective.label(),
                    pos + 1
                ),
            });
        }

        check_value(
            card_no,
            l.line,
            &name,
            head,
            value,
            effective,
            region,
            check_email,
            check_phone,
            issues,
        );
    }

    // Cardinality: at most one of each single-instance property.
    for name in &seen {
        let n = counts[name];
        if n > 1 && SINGLE_INSTANCE.contains(&name.as_str()) {
            issues.push(Issue {
                line: card.begin_line,
                card: card_no,
                severity: Severity::Error,
                rule: "duplicate-property",
                property: name.clone(),
                message: format!(
                    "{name} may appear at most once per card but appears {n} times"
                ),
            });
        }
    }

    // Version-specific required properties.
    if !counts.contains_key("VERSION") {
        issues.push(Issue {
            line: card.begin_line,
            card: card_no,
            severity: Severity::Error,
            rule: "missing-version",
            property: "VERSION".into(),
            message: "every vCard must carry a VERSION property (2.1, 3.0 or 4.0); this card has none".into(),
        });
    }
    if matches!(effective, Version::V30 | Version::V40) && !counts.contains_key("FN") {
        issues.push(Issue {
            line: card.begin_line,
            card: card_no,
            severity: Severity::Error,
            rule: "missing-fn",
            property: "FN".into(),
            message: format!(
                "FN (the formatted display name) is required in vCard {} but this card has none",
                effective.label()
            ),
        });
    }
    if matches!(effective, Version::V21 | Version::V30) && !counts.contains_key("N") {
        issues.push(Issue {
            line: card.begin_line,
            card: card_no,
            severity: Severity::Error,
            rule: "missing-n",
            property: "N".into(),
            message: format!(
                "N (the structured name) is required in vCard {} but this card has none",
                effective.label()
            ),
        });
    }
    for want in required {
        if !counts.contains_key(want) {
            issues.push(Issue {
                line: card.begin_line,
                card: card_no,
                severity: Severity::Error,
                rule: "missing-required-property",
                property: want.clone(),
                message: format!(
                    "{want} was listed in required properties but this card does not have it"
                ),
            });
        }
    }
}

/// Which spec version this card is actually checked against, reporting an
/// unknown or mismatched `VERSION` on the way.
fn effective_version(
    card: &Card,
    expected: Version,
    card_no: usize,
    issues: &mut Vec<Issue>,
) -> Version {
    let declared = match card.version_value.as_deref() {
        Some(v) => match Version::parse(v) {
            Ok(Version::Auto) | Err(_) => {
                issues.push(Issue {
                    line: card.begin_line,
                    card: card_no,
                    severity: Severity::Error,
                    rule: "unknown-version",
                    property: "VERSION".into(),
                    message: format!(
                        "VERSION value '{}' is not a known vCard version (expected 2.1, 3.0 or 4.0)",
                        truncate(v.trim(), 30)
                    ),
                });
                None
            }
            Ok(known) => Some(known),
        },
        None => None,
    };

    match expected {
        // Explicit target: report any card that declares something else.
        Version::Auto => declared.unwrap_or(Version::V30),
        want => {
            if let Some(d) = declared {
                if d != want {
                    issues.push(Issue {
                        line: card.begin_line,
                        card: card_no,
                        severity: Severity::Error,
                        rule: "version-mismatch",
                        property: "VERSION".into(),
                        message: format!(
                            "card declares VERSION {} but was checked against vCard {}",
                            d.label(),
                            want.label()
                        ),
                    });
                }
            }
            want
        }
    }
}

/// Property names are `[A-Za-z0-9-]+`, optionally prefixed by a `group.`.
fn check_property_name(
    card_no: usize,
    line: usize,
    head: &str,
    name: &str,
    issues: &mut Vec<Issue>,
) {
    if name.is_empty() {
        issues.push(Issue {
            line,
            card: card_no,
            severity: Severity::Error,
            rule: "empty-property-name",
            property: String::new(),
            message: "content line has an empty property name before the ':'".into(),
        });
        return;
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        issues.push(Issue {
            line,
            card: card_no,
            severity: Severity::Error,
            rule: "invalid-property-name",
            property: name.to_string(),
            message: format!(
                "property name '{}' contains characters outside A-Z, 0-9 and '-'",
                truncate(name, 30)
            ),
        });
        return;
    }
    // Group prefix (`item1.TEL:`) must itself be alphanumeric/'-'.
    let group_part = head.split(';').next().unwrap_or(head);
    if let Some((group, _)) = group_part.rsplit_once('.') {
        if group.is_empty() || !group.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            issues.push(Issue {
                line,
                card: card_no,
                severity: Severity::Error,
                rule: "invalid-group",
                property: name.to_string(),
                message: format!(
                    "group prefix '{}' is not a valid group name (A-Z, 0-9 and '-' only)",
                    truncate(group, 30)
                ),
            });
        }
    }
    if !KNOWN_PROPERTIES.contains(&name) && !name.starts_with("X-") {
        issues.push(Issue {
            line,
            card: card_no,
            severity: Severity::Warning,
            rule: "unknown-property",
            property: name.to_string(),
            message: format!(
                "{name} is not a property defined by vCard 2.1, 3.0 or 4.0; non-standard properties should use an 'X-' prefix"
            ),
        });
    }
}

/// Parameter-form rules: bare (value-only) parameters are legal in 2.1 only,
/// and a parameter value containing `,`, `;` or `:` must be double-quoted.
fn check_parameters(
    card_no: usize,
    line: usize,
    head: &str,
    name: &str,
    version: Version,
    issues: &mut Vec<Issue>,
) {
    for param in split_params(head) {
        let param = param.trim();
        if param.is_empty() {
            issues.push(Issue {
                line,
                card: card_no,
                severity: Severity::Error,
                rule: "empty-parameter",
                property: name.to_string(),
                message: "empty parameter (two ';' separators in a row, or a trailing ';')".into(),
            });
            continue;
        }
        match param.split_once('=') {
            None => {
                if version == Version::V21 {
                    continue; // `TEL;WORK;VOICE:` is the normal 2.1 form.
                }
                issues.push(Issue {
                    line,
                    card: card_no,
                    severity: Severity::Error,
                    rule: "bare-parameter",
                    property: name.to_string(),
                    message: format!(
                        "parameter '{}' has no '=' — the bare form is vCard 2.1 only; in {} write it as TYPE={}",
                        truncate(param, 30),
                        version.label(),
                        truncate(param, 30)
                    ),
                });
            }
            Some((pname, pvalue)) => {
                if pname.trim().is_empty() {
                    issues.push(Issue {
                        line,
                        card: card_no,
                        severity: Severity::Error,
                        rule: "empty-parameter",
                        property: name.to_string(),
                        message: "parameter has an empty name before '='".into(),
                    });
                    continue;
                }
                let unquoted = !(pvalue.starts_with('"') && pvalue.ends_with('"'));
                if unquoted && pvalue.contains(':') {
                    issues.push(Issue {
                        line,
                        card: card_no,
                        severity: Severity::Warning,
                        rule: "unquoted-parameter",
                        property: name.to_string(),
                        message: format!(
                            "parameter {} has an unquoted value containing ':'; wrap it in double quotes so parsers do not split the line early",
                            pname.trim().to_ascii_uppercase()
                        ),
                    });
                }
                if pname.trim().eq_ignore_ascii_case("CHARSET") && version != Version::V21 {
                    issues.push(Issue {
                        line,
                        card: card_no,
                        severity: Severity::Warning,
                        rule: "charset-parameter",
                        property: name.to_string(),
                        message: format!(
                            "CHARSET is a vCard 2.1 parameter; vCard {} documents are always UTF-8 and should drop it",
                            version.label()
                        ),
                    });
                }
            }
        }
    }
}

/// Value-level rules, dispatched on the property name.
#[allow(clippy::too_many_arguments)]
fn check_value(
    card_no: usize,
    line: usize,
    name: &str,
    head: &str,
    value: &str,
    version: Version,
    region: Option<country::Id>,
    check_email: bool,
    check_phone: bool,
    issues: &mut Vec<Issue>,
) {
    let v = value.trim();
    let mut push = |severity: Severity, rule: &'static str, message: String| {
        issues.push(Issue {
            line,
            card: card_no,
            severity,
            rule,
            property: name.to_string(),
            message,
        })
    };

    if v.is_empty() && !matches!(name, "N" | "ADR" | "") {
        push(
            Severity::Warning,
            "empty-value",
            format!("{name} has an empty value; drop the property instead of emitting it empty"),
        );
        return;
    }

    match name {
        "EMAIL" if check_email => {
            if let Err(why) = email_problem(v) {
                push(
                    Severity::Error,
                    "invalid-email",
                    format!("EMAIL value '{}' is not a valid address: {why}", truncate(v, 40)),
                );
            }
        }
        "TEL" if check_phone => {
            let raw = v.strip_prefix("tel:").unwrap_or(v);
            // Strip a `;ext=`/`;phone-context=` URI suffix before parsing.
            let raw = raw.split(';').next().unwrap_or(raw).trim();
            if raw.chars().any(|c| c.is_ascii_digit()) {
                match phonenumber::parse(region, raw) {
                    Ok(n) if phonenumber::is_valid(&n) => {}
                    Ok(_) => push(
                        Severity::Error,
                        "invalid-tel",
                        format!(
                            "TEL value '{}' parses but is not a valid number for its country{}",
                            truncate(raw, 40),
                            region_note(region)
                        ),
                    ),
                    Err(_) if !raw.starts_with('+') && region.is_none() => push(
                        Severity::Warning,
                        "unverifiable-tel",
                        format!(
                            "TEL value '{}' is in national format and cannot be checked without a country; set a default country or write it as +<country code>...",
                            truncate(raw, 40)
                        ),
                    ),
                    Err(e) => push(
                        Severity::Error,
                        "invalid-tel",
                        format!(
                            "TEL value '{}' is not a parseable phone number ({e}){}",
                            truncate(raw, 40),
                            region_note(region)
                        ),
                    ),
                }
            } else {
                push(
                    Severity::Error,
                    "invalid-tel",
                    format!("TEL value '{}' contains no digits", truncate(v, 40)),
                );
            }
            if version == Version::V40 && !v.starts_with("tel:") {
                push(
                    Severity::Warning,
                    "tel-not-uri",
                    "in vCard 4.0 a TEL value should be a 'tel:' URI (for example tel:+15551234567) unless VALUE=text is set".into(),
                );
            }
        }
        "N" => {
            let n = component_count(v);
            if n != 5 {
                push(
                    Severity::Error,
                    "invalid-n",
                    format!(
                        "N must have exactly 5 semicolon-separated components (family;given;additional;prefixes;suffixes) but has {n}"
                    ),
                );
            }
        }
        "ADR" => {
            let n = component_count(v);
            if n != 7 {
                push(
                    Severity::Error,
                    "invalid-adr",
                    format!(
                        "ADR must have exactly 7 semicolon-separated components (po-box;extended;street;locality;region;postal-code;country) but has {n}"
                    ),
                );
            }
        }
        "BDAY" | "ANNIVERSARY" => {
            if !is_date_value(v, version) {
                push(
                    Severity::Error,
                    "invalid-date",
                    format!(
                        "{name} value '{}' is not a valid date{}",
                        truncate(v, 40),
                        if version == Version::V40 {
                            " (expected YYYYMMDD, YYYY-MM-DD, or a vCard 4.0 partial date such as --0415)"
                        } else {
                            " (expected YYYY-MM-DD or YYYYMMDD)"
                        }
                    ),
                );
            }
        }
        "REV" => {
            if !is_timestamp_value(v) {
                push(
                    Severity::Error,
                    "invalid-date",
                    format!(
                        "REV value '{}' is not a valid timestamp (expected YYYYMMDDTHHMMSSZ or YYYY-MM-DDTHH:MM:SSZ)",
                        truncate(v, 40)
                    ),
                );
            }
        }
        "URL" | "SOURCE" | "FBURL" | "CALURI" | "CALADRURI" => {
            if !is_uri(v) {
                push(
                    Severity::Warning,
                    "invalid-uri",
                    format!(
                        "{name} value '{}' is not an absolute URI (expected a scheme such as https://)",
                        truncate(v, 40)
                    ),
                );
            }
        }
        "KIND" if version == Version::V40 => {
            let k = v.to_ascii_lowercase();
            if !matches!(k.as_str(), "individual" | "group" | "org" | "location")
                && !k.starts_with("x-")
            {
                push(
                    Severity::Error,
                    "invalid-kind",
                    format!(
                        "KIND value '{}' is not one of individual, group, org or location",
                        truncate(v, 30)
                    ),
                );
            }
        }
        "GENDER" if version == Version::V40 => {
            let sex = v.split(';').next().unwrap_or("").trim().to_ascii_uppercase();
            if !matches!(sex.as_str(), "" | "M" | "F" | "O" | "N" | "U") {
                push(
                    Severity::Error,
                    "invalid-gender",
                    format!(
                        "GENDER sex component '{}' is not one of M, F, O, N, U (or empty)",
                        truncate(&sex, 20)
                    ),
                );
            }
        }
        "PHOTO" | "LOGO" | "SOUND" | "KEY" if version == Version::V40 => {
            // In 4.0 these are URIs (often `data:`); in 2.1/3.0 they were
            // base64 blobs, so only check the 4.0 form.
            if !head.to_ascii_uppercase().contains("ENCODING=") && !is_uri(v) {
                push(
                    Severity::Warning,
                    "invalid-uri",
                    format!(
                        "{name} value should be a URI in vCard 4.0 (for example a https:// or data: URI), got '{}'",
                        truncate(v, 40)
                    ),
                );
            }
        }
        _ => {}
    }
}

/// Human-readable, grouped by card.
fn render_report(cards: &[Card], issues: &[Issue], expected: Version) -> String {
    let errors = issues.iter().filter(|i| i.severity == Severity::Error).count();
    let warnings = issues.len() - errors;

    let mut out = String::new();
    out.push_str(&format!(
        "{} — {} card{}, {} error{}, {} warning{}\n",
        if errors == 0 { "VALID" } else { "INVALID" },
        cards.len(),
        plural(cards.len()),
        errors,
        plural(errors),
        warnings,
        plural(warnings),
    ));
    out.push_str(&format!(
        "Checked against: {}\n",
        match expected {
            Version::Auto => "each card's own VERSION (auto)".to_string(),
            v => format!("vCard {}", v.label()),
        }
    ));

    let doc: Vec<&Issue> = issues.iter().filter(|i| i.card == 0).collect();
    if !doc.is_empty() {
        out.push('\n');
        out.push_str("Document\n");
        for i in &doc {
            out.push_str(&format_issue(i));
        }
    }

    for (idx, card) in cards.iter().enumerate() {
        let no = idx + 1;
        let mine: Vec<&Issue> = issues.iter().filter(|i| i.card == no).collect();
        let ver = card.version_value.as_deref().unwrap_or("none");
        out.push('\n');
        out.push_str(&format!(
            "Card {no} (line {}, VERSION {ver}, {} propert{})\n",
            card.begin_line,
            card.lines.len(),
            if card.lines.len() == 1 { "y" } else { "ies" }
        ));
        if mine.is_empty() {
            out.push_str("  no issues\n");
        } else {
            for i in &mine {
                out.push_str(&format_issue(i));
            }
        }
    }
    out
}

fn format_issue(i: &Issue) -> String {
    let loc = if i.line == 0 {
        "     -".to_string()
    } else {
        format!("{:>6}", i.line)
    };
    format!(
        "  line {loc}  {:<7}  {:<26}  {}\n",
        i.severity.as_str(),
        i.rule,
        i.message
    )
}

/// Structured output for CI.
fn render_json(cards: &[Card], issues: &[Issue]) -> String {
    let errors = issues.iter().filter(|i| i.severity == Severity::Error).count();
    let warnings = issues.len() - errors;

    let mut s = String::new();
    s.push_str("{\n");
    s.push_str(&format!("  \"ok\": {},\n", errors == 0));
    s.push_str(&format!("  \"cards\": {},\n", cards.len()));
    s.push_str(&format!("  \"error_count\": {errors},\n"));
    s.push_str(&format!("  \"warning_count\": {warnings},\n"));
    s.push_str("  \"versions\": [");
    for (idx, c) in cards.iter().enumerate() {
        if idx > 0 {
            s.push_str(", ");
        }
        match c.version_value.as_deref() {
            Some(v) => s.push_str(&format!("\"{}\"", json_escape(v))),
            None => s.push_str("null"),
        }
    }
    s.push_str("],\n");
    s.push_str("  \"issues\": [");
    for (idx, i) in issues.iter().enumerate() {
        s.push_str(if idx == 0 { "\n" } else { ",\n" });
        s.push_str(&format!(
            "    {{\"card\": {}, \"line\": {}, \"severity\": \"{}\", \"rule\": \"{}\", \"property\": \"{}\", \"message\": \"{}\"}}",
            i.card,
            i.line,
            i.severity.as_str(),
            i.rule,
            json_escape(&i.property),
            json_escape(&i.message)
        ));
    }
    if !issues.is_empty() {
        s.push('\n');
        s.push_str("  ");
    }
    s.push_str("]\n}");
    s
}

// ---------------------------------------------------------------- helpers

/// Split `NAME[;PARAMS]:VALUE` at the first colon that is not inside a quoted
/// parameter value. Returns `(head, value)`.
fn split_line(line: &str) -> Option<(&str, &str)> {
    let bytes = line.as_bytes();
    let mut in_quotes = false;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'"' => in_quotes = !in_quotes,
            b':' if !in_quotes => return Some((&line[..i], &line[i + 1..])),
            _ => {}
        }
    }
    None
}

/// The property name from a head, dropping any `group.` prefix and params.
fn property_name(head: &str) -> &str {
    let first = head.split(';').next().unwrap_or(head);
    match first.rsplit_once('.') {
        Some((_, name)) => name,
        None => first,
    }
}

/// Parameter segments of a head (everything after the first unquoted ';').
fn split_params(head: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = Vec::new();
    let mut in_quotes = false;
    let mut start = None;
    for (i, c) in head.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            ';' if !in_quotes => {
                if let Some(s) = start {
                    parts.push(&head[s..i]);
                }
                start = Some(i + 1);
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        parts.push(&head[s..]);
    }
    parts
}

/// Count semicolon-separated components, honouring `\;` escapes.
fn component_count(value: &str) -> usize {
    let mut n = 1;
    let mut escaped = false;
    for c in value.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            ';' => n += 1,
            _ => {}
        }
    }
    n
}

/// A pragmatic address-syntax check: one `@`, a non-empty local part with no
/// spaces or control characters, and a dotted domain of valid labels.
fn email_problem(v: &str) -> Result<(), String> {
    if v.chars().any(|c| c.is_whitespace()) {
        return Err("it contains whitespace".into());
    }
    let (local, domain) = match v.split_once('@') {
        Some(p) => p,
        None => return Err("it has no '@'".into()),
    };
    if domain.contains('@') {
        return Err("it has more than one '@'".into());
    }
    if local.is_empty() {
        return Err("the part before '@' is empty".into());
    }
    if local.len() > 64 {
        return Err("the part before '@' is longer than 64 characters".into());
    }
    if local.starts_with('.') || local.ends_with('.') || local.contains("..") {
        return Err("the part before '@' has a leading, trailing or doubled '.'".into());
    }
    if domain.is_empty() {
        return Err("the domain is empty".into());
    }
    if !domain.contains('.') {
        return Err("the domain has no dot (a top-level domain is required)".into());
    }
    for label in domain.split('.') {
        if label.is_empty() {
            return Err("the domain has an empty label (a leading, trailing or doubled '.')".into());
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err(format!("domain label '{label}' starts or ends with '-'"));
        }
        if !label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || !c.is_ascii())
        {
            return Err(format!("domain label '{label}' has an invalid character"));
        }
    }
    Ok(())
}

/// `YYYYMMDD` / `YYYY-MM-DD`, plus vCard 4.0 partial dates (`YYYY`, `YYYY-MM`,
/// `--MMDD`, `--MM-DD`, `---DD`) and an optional time part.
fn is_date_value(v: &str, version: Version) -> bool {
    let date = v.split(['T', 't']).next().unwrap_or(v);
    let d: String = date.chars().filter(|c| *c != '-').collect();
    let dashes = date.len() - d.len();

    if version == Version::V40 {
        if date.starts_with("---") {
            return d.len() == 2 && all_digits(&d);
        }
        if date.starts_with("--") {
            return matches!(d.len(), 2 | 4) && all_digits(&d);
        }
        if d.len() == 4 && dashes == 0 {
            return all_digits(&d);
        }
        if d.len() == 6 && dashes == 1 {
            return all_digits(&d) && valid_month(&d[4..6]);
        }
    }
    d.len() == 8 && all_digits(&d) && valid_month(&d[4..6]) && valid_day(&d[6..8])
}

/// `YYYYMMDDTHHMMSS[Z|±HHMM]`, dashes/colons optional.
fn is_timestamp_value(v: &str) -> bool {
    let (date, rest) = match v.split_once(['T', 't']) {
        Some(p) => p,
        None => return false,
    };
    if !is_date_value(date, Version::V30) {
        return false;
    }
    let time = rest
        .trim_end_matches(['Z', 'z'])
        .split(['+', '-'])
        .next()
        .unwrap_or(rest);
    let t: String = time.chars().filter(|c| *c != ':').collect();
    matches!(t.len(), 4 | 6) && all_digits(&t)
}

/// An absolute URI: `scheme:` where the scheme starts with a letter.
fn is_uri(v: &str) -> bool {
    match v.split_once(':') {
        Some((scheme, rest)) => {
            !rest.is_empty()
                && !scheme.is_empty()
                && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        }
        None => false,
    }
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

fn valid_month(s: &str) -> bool {
    matches!(s.parse::<u32>(), Ok(m) if (1..=12).contains(&m))
}

fn valid_day(s: &str) -> bool {
    matches!(s.parse::<u32>(), Ok(d) if (1..=31).contains(&d))
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

fn region_note(region: Option<country::Id>) -> String {
    match region {
        // `country::Id` implements Debug (printing the alpha-2 code), not Display.
        Some(r) => format!(" (checked as {r:?})"),
        None => String::new(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let head: String = s.chars().take(max).collect();
    format!("{head}...")
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
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
    out
}

/// Resolve an ISO-3166 alpha-2 region hint for phone-number parsing.
fn parse_region(code: &str) -> Result<Option<country::Id>, String> {
    let c = code.trim();
    if c.is_empty() {
        return Ok(None);
    }
    c.to_ascii_uppercase()
        .parse::<country::Id>()
        .map(Some)
        .map_err(|_| {
            format!("unknown default_country '{c}': expected an ISO-3166 alpha-2 code such as US, GB or DE")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Ada Lovelace\r\nN:Lovelace;Ada;;;\r\nEMAIL:ada@example.com\r\nTEL:+16502530000\r\nEND:VCARD\r\n";

    fn run(data: &str) -> String {
        validate(data, Version::Auto, "", true, true, "", Output::Report).unwrap()
    }

    #[test]
    fn happy_path_valid_card_reports_no_issues() {
        let out = run(GOOD);
        assert!(out.starts_with("VALID — 1 card, 0 errors, 0 warnings"), "{out}");
        assert!(out.contains("Card 1 (line 1, VERSION 3.0, 5 properties)"), "{out}");
        assert!(out.contains("no issues"), "{out}");
    }

    #[test]
    fn error_no_vcard_in_input() {
        let err = validate("hello world", Version::Auto, "", true, true, "", Output::Report)
            .unwrap_err();
        assert!(err.contains("no vCard found"), "{err}");
    }

    #[test]
    fn error_unknown_default_country() {
        let err = validate(GOOD, Version::Auto, "ZZZ", true, true, "", Output::Report).unwrap_err();
        assert!(err.contains("unknown default_country 'ZZZ'"), "{err}");
    }

    #[test]
    fn flags_missing_fn_and_missing_version() {
        let out = run("BEGIN:VCARD\r\nN:Doe;Jane;;;\r\nEND:VCARD\r\n");
        assert!(out.contains("missing-version"), "{out}");
        assert!(out.contains("missing-fn"), "{out}");
        assert!(out.starts_with("INVALID"), "{out}");
    }

    #[test]
    fn flags_bad_email_and_bad_phone() {
        let out = run(
            "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:X\r\nN:X;;;;\r\nEMAIL:not-an-email\r\nTEL:+15550000000\r\nEND:VCARD\r\n",
        );
        assert!(out.contains("invalid-email"), "{out}");
        assert!(out.contains("it has no '@'"), "{out}");
        assert!(out.contains("invalid-tel"), "{out}");
    }

    #[test]
    fn email_check_can_be_disabled() {
        let data =
            "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:X\r\nN:X;;;;\r\nEMAIL:not-an-email\r\nEND:VCARD\r\n";
        let on = validate(data, Version::Auto, "", true, true, "", Output::Report).unwrap();
        let off = validate(data, Version::Auto, "", false, true, "", Output::Report).unwrap();
        assert!(on.contains("invalid-email"), "{on}");
        assert!(!off.contains("invalid-email"), "{off}");
    }

    #[test]
    fn national_phone_needs_a_country_hint() {
        let data = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:X\r\nN:X;;;;\r\nTEL:(650) 253-0000\r\nEND:VCARD\r\n";
        let without = run(data);
        assert!(without.contains("unverifiable-tel"), "{without}");
        let with = validate(data, Version::Auto, "US", true, true, "", Output::Report).unwrap();
        assert!(!with.contains("tel"), "{with}");
    }

    #[test]
    fn flags_wrong_component_counts() {
        let out = run("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:X\r\nN:Doe;Jane\r\nADR:;;1 Main St;Springfield\r\nEND:VCARD\r\n");
        assert!(out.contains("invalid-n"), "{out}");
        assert!(out.contains("but has 2"), "{out}");
        assert!(out.contains("invalid-adr"), "{out}");
        assert!(out.contains("but has 4"), "{out}");
    }

    #[test]
    fn escaped_semicolons_do_not_count_as_components() {
        let out = run("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:X\r\nN:de la Cruz\\; Jr.;Ana;;;\r\nEND:VCARD\r\n");
        assert!(!out.contains("invalid-n"), "{out}");
    }

    #[test]
    fn version_40_requires_version_first_and_tel_uri() {
        let out = run("BEGIN:VCARD\r\nFN:X\r\nVERSION:4.0\r\nTEL:+16502530000\r\nEND:VCARD\r\n");
        assert!(out.contains("version-not-first"), "{out}");
        assert!(out.contains("tel-not-uri"), "{out}");
        // N is optional in 4.0.
        assert!(!out.contains("missing-n"), "{out}");
    }

    #[test]
    fn version_mismatch_when_an_explicit_target_is_given() {
        let out = validate(GOOD, Version::V40, "", true, true, "", Output::Report).unwrap();
        assert!(out.contains("version-mismatch"), "{out}");
        assert!(out.contains("Checked against: vCard 4.0"), "{out}");
    }

    #[test]
    fn bare_parameters_are_ok_in_21_but_not_30() {
        let card = |v: &str| {
            format!("BEGIN:VCARD\r\nVERSION:{v}\r\nFN:X\r\nN:X;;;;\r\nTEL;WORK:+16502530000\r\nEND:VCARD\r\n")
        };
        assert!(!run(&card("2.1")).contains("bare-parameter"));
        assert!(run(&card("3.0")).contains("bare-parameter"));
    }

    #[test]
    fn flags_structural_problems() {
        let out = validate(
            "FN:orphan\r\nBEGIN:VCARD\r\nVERSION:3.0\r\nFN:X\r\nN:X;;;;\r\nEND:VCARD\r\nEND:VCARD\r\n",
            Version::Auto,
            "",
            true,
            true,
            "",
            Output::Report,
        )
        .unwrap();
        assert!(out.contains("content-outside-card"), "{out}");
        assert!(out.contains("stray-end"), "{out}");
    }

    #[test]
    fn unclosed_card_is_reported() {
        let out = run("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:X\r\nN:X;;;;\r\n");
        assert!(out.contains("unclosed-card"), "{out}");
    }

    #[test]
    fn duplicate_single_instance_property() {
        let out = run("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:X\r\nN:A;;;;\r\nN:B;;;;\r\nEND:VCARD\r\n");
        assert!(out.contains("duplicate-property"), "{out}");
        assert!(out.contains("appears 2 times"), "{out}");
    }

    #[test]
    fn folded_lines_are_unfolded_before_checking() {
        // A folded FN must not be seen as a stray fold or a missing FN.
        let out = run("BEGIN:VCARD\r\nVERSION:3.0\r\nN:X;;;;\r\nFN:Ada\r\n  Lovelace\r\nEND:VCARD\r\n");
        assert!(!out.contains("missing-fn"), "{out}");
        assert!(!out.contains("stray-fold"), "{out}");
    }

    #[test]
    fn required_properties_are_enforced() {
        let out = validate(GOOD, Version::Auto, "", true, true, "UID, ORG", Output::Report).unwrap();
        assert!(out.contains("missing-required-property"), "{out}");
        assert!(out.contains("UID was listed"), "{out}");
        assert!(out.contains("ORG was listed"), "{out}");
    }

    #[test]
    fn unknown_property_is_a_warning_and_x_prefix_is_not() {
        let out = run("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:X\r\nN:X;;;;\r\nWIBBLE:1\r\nX-CUSTOM:2\r\nEND:VCARD\r\n");
        assert!(out.contains("unknown-property"), "{out}");
        assert!(out.contains("WIBBLE"), "{out}");
        assert_eq!(out.matches("unknown-property").count(), 1, "{out}");
    }

    #[test]
    fn flags_bad_dates_and_uris() {
        let out = run("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:X\r\nN:X;;;;\r\nBDAY:1815-13-40\r\nURL:example.com\r\nEND:VCARD\r\n");
        assert!(out.contains("invalid-date"), "{out}");
        assert!(out.contains("invalid-uri"), "{out}");
    }

    #[test]
    fn vcard_40_partial_dates_are_accepted() {
        let out = run("BEGIN:VCARD\r\nVERSION:4.0\r\nFN:X\r\nBDAY:--0415\r\nANNIVERSARY:1996\r\nEND:VCARD\r\n");
        assert!(!out.contains("invalid-date"), "{out}");
        // ...but not in 3.0.
        let out30 = run("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:X\r\nN:X;;;;\r\nBDAY:--0415\r\nEND:VCARD\r\n");
        assert!(out30.contains("invalid-date"), "{out30}");
    }

    #[test]
    fn missing_colon_is_an_error() {
        let out = run("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:X\r\nN:X;;;;\r\nTHIS IS NOT A LINE\r\nEND:VCARD\r\n");
        assert!(out.contains("missing-colon"), "{out}");
    }

    #[test]
    fn lf_only_document_gets_a_warning() {
        let out = run("BEGIN:VCARD\nVERSION:3.0\nFN:X\nN:X;;;;\nEND:VCARD\n");
        assert!(out.contains("lf-line-endings"), "{out}");
    }

    #[test]
    fn json_output_is_parseable_and_counts_match() {
        let out = validate(
            "BEGIN:VCARD\r\nVERSION:3.0\r\nEND:VCARD\r\n",
            Version::Auto,
            "",
            true,
            true,
            "",
            Output::Json,
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["cards"], 1);
        assert_eq!(v["error_count"], 2); // missing-fn + missing-n
        assert_eq!(v["versions"][0], "3.0");
        assert_eq!(v["issues"][0]["rule"], "missing-fn");
    }

    #[test]
    fn json_output_of_a_clean_document_has_an_empty_issue_list() {
        let out =
            validate(GOOD, Version::Auto, "", true, true, "", Output::Json).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["issues"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn multiple_cards_are_reported_separately() {
        let two = format!("{GOOD}{}", GOOD.replace("VERSION:3.0", "VERSION:9.9"));
        let out = run(&two);
        assert!(out.contains("Card 1 "), "{out}");
        assert!(out.contains("Card 2 "), "{out}");
        assert!(out.contains("unknown-version"), "{out}");
    }

    #[test]
    fn parse_helpers_reject_bad_arguments() {
        assert!(Version::parse("5.0").unwrap_err().contains("unknown version"));
        assert_eq!(Version::parse("").unwrap(), Version::Auto);
        assert!(Output::parse("yaml").unwrap_err().contains("unknown output"));
        assert_eq!(Output::parse("json").unwrap(), Output::Json);
    }
}
