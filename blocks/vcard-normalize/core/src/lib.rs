//! vcard-normalize core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Takes raw vCard (.vcf) text — one or many `BEGIN:VCARD … END:VCARD` blocks,
//! vCard 2.1 / 3.0 (RFC 2426) / 4.0 (RFC 6350), including folded lines — and
//! returns normalized vCard text with:
//!   * `EMAIL` values trimmed and (by default) lowercased,
//!   * `TEL` values reformatted to E.164 when they parse and validate, using an
//!     optional `default_country` region hint (extension preserved as `;ext=`),
//!   * `FN` / `N` / `NICKNAME` whitespace tidied and optionally recased.
//! Every other property (VERSION, ORG, ADR, PHOTO, X-*, TYPE params, …) and the
//! card structure are preserved verbatim. See the tool's competitor-analysis doc
//! for the full list of documented limits.

use phonenumber::country;
use phonenumber::Mode;

/// How to recase name components in `FN`, `N`, and `NICKNAME`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameCase {
    /// Only tidy whitespace; leave the existing case untouched (default).
    Keep,
    /// Capitalize the first letter of each whitespace-delimited word.
    Title,
    /// Uppercase the whole value.
    Upper,
    /// Lowercase the whole value.
    Lower,
}

impl NameCase {
    /// Parse the `name_case` argument. Empty/whitespace → `Keep`.
    pub fn parse(s: &str) -> Result<NameCase, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "keep" => Ok(NameCase::Keep),
            "title" => Ok(NameCase::Title),
            "upper" => Ok(NameCase::Upper),
            "lower" => Ok(NameCase::Lower),
            other => Err(format!(
                "unknown name_case '{other}' (use 'keep', 'title', 'upper', or 'lower')"
            )),
        }
    }
}

/// Normalize a vCard document.
///
/// - `default_country`: ISO-3166 alpha-2 region (e.g. `US`, `GB`) used to
///   interpret `TEL` values written without a `+` prefix. Empty = no region hint
///   (only `+`-prefixed numbers can be reformatted). An unrecognised code errors.
/// - `name_case`: how to recase `FN` / `N` / `NICKNAME` components.
/// - `lowercase_email`: lowercase `EMAIL` values (in addition to trimming).
///
/// Returns the normalized vCard text, or `Err` when no card is found or the
/// region hint is invalid.
pub fn run(
    input: &str,
    default_country: &str,
    name_case: NameCase,
    lowercase_email: bool,
) -> Result<String, String> {
    // Resolve the region hint once, up front, so a bad code fails fast.
    let region = parse_region(default_country)?;

    // Preserve the document's line-ending style: if the input uses CRLF
    // anywhere, emit CRLF; otherwise LF.
    let nl = if input.contains("\r\n") { "\r\n" } else { "\n" };

    let logical = unfold(input);
    let mut out = String::new();
    let mut in_card = false;
    let mut seen_card = false;

    for raw in &logical {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let upper = trimmed.to_ascii_uppercase();
        if upper == "BEGIN:VCARD" {
            in_card = true;
            seen_card = true;
            out.push_str("BEGIN:VCARD");
            out.push_str(nl);
            continue;
        }
        if upper == "END:VCARD" {
            in_card = false;
            out.push_str("END:VCARD");
            out.push_str(nl);
            continue;
        }
        if !in_card {
            // Stray line outside any card — ignore (matches sibling vCard tools).
            continue;
        }
        out.push_str(&transform_line(trimmed, region, name_case, lowercase_email));
        out.push_str(nl);
    }

    if !seen_card {
        return Err(
            "no vCard found: expected at least one 'BEGIN:VCARD ... END:VCARD' block".into(),
        );
    }
    Ok(out)
}

/// Transform one content line `[group.]NAME[;params]:value`, rewriting only the
/// value and only for the properties we normalize. The head (group + name +
/// params) is preserved verbatim. A line with no ':' is returned unchanged.
fn transform_line(
    line: &str,
    region: Option<country::Id>,
    name_case: NameCase,
    lowercase_email: bool,
) -> String {
    let colon = match find_unquoted_colon(line) {
        Some(i) => i,
        None => return line.to_string(),
    };
    let head = &line[..colon];
    let value = &line[colon + 1..];

    // Property name: last '.'-segment of the first ';'-part, upper-cased.
    let name_tok = head.split(';').next().unwrap_or(head);
    let prop = name_tok
        .rsplit('.')
        .next()
        .unwrap_or(name_tok)
        .trim()
        .to_ascii_uppercase();

    let new_value = match prop.as_str() {
        "EMAIL" => normalize_email(value, lowercase_email),
        "TEL" => normalize_phone(value, region),
        "FN" => tidy_and_case(value, name_case),
        "N" => normalize_structured(value, ';', name_case),
        "NICKNAME" => normalize_structured(value, ',', name_case),
        _ => value.to_string(),
    };

    format!("{head}:{new_value}")
}

/// Trim, and (when `lowercase`) lowercase an EMAIL value.
fn normalize_email(value: &str, lowercase: bool) -> String {
    let v = value.trim();
    if lowercase {
        v.to_ascii_lowercase()
    } else {
        v.to_string()
    }
}

/// Reformat a TEL value to E.164 when it parses AND validates for the given
/// region. Otherwise the original value is returned untouched (conservative — a
/// wrong E.164 is worse than an untouched one). A parsed extension is preserved
/// as `;ext=<digits>`.
fn normalize_phone(value: &str, region: Option<country::Id>) -> String {
    let v = value.trim();
    if v.is_empty() {
        return value.to_string();
    }
    match phonenumber::parse(region, v) {
        Ok(parsed) if phonenumber::is_valid(&parsed) => {
            let mut s = parsed.format().mode(Mode::E164).to_string();
            if let Some(ext) = parsed.extension() {
                s.push_str(";ext=");
                s.push_str(ext.as_ref());
            }
            s
        }
        _ => value.to_string(),
    }
}

/// Tidy whitespace and apply the case transform to a free-text value (`FN`).
fn tidy_and_case(value: &str, case: NameCase) -> String {
    apply_case(&collapse_ws(value), case)
}

/// Normalize a structured, `sep`-delimited value (`N` uses `;`, `NICKNAME` uses
/// `,`): tidy + recase each component, preserving the separators (and any empty
/// components) and unescaped positions.
fn normalize_structured(value: &str, sep: char, case: NameCase) -> String {
    let parts = split_unescaped(value, sep);
    let normalized: Vec<String> = parts.iter().map(|p| tidy_and_case(p, case)).collect();
    normalized.join(&sep.to_string())
}

/// Apply a `NameCase` to already-whitespace-tidied text.
fn apply_case(s: &str, case: NameCase) -> String {
    match case {
        NameCase::Keep => s.to_string(),
        NameCase::Upper => s.to_uppercase(),
        NameCase::Lower => s.to_lowercase(),
        NameCase::Title => title_case(s),
    }
}

/// Naive title-case: uppercase the first character of each whitespace-delimited
/// word, lowercase the rest. (Documented limit: mangles `McDonald` → `Mcdonald`.)
fn title_case(s: &str) -> String {
    s.split(' ')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Collapse runs of ASCII whitespace to a single space and trim the ends.
fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parse an optional ISO-3166 alpha-2 region hint. Empty/whitespace → `None`.
/// An unrecognised code is an error (so a typo doesn't silently disable
/// phone normalization).
fn parse_region(region: &str) -> Result<Option<country::Id>, String> {
    let region = region.trim();
    if region.is_empty() {
        return Ok(None);
    }
    region
        .to_ascii_uppercase()
        .parse::<country::Id>()
        .map(Some)
        .map_err(|_| {
            format!(
                "unrecognised default_country {region:?} (expected ISO-3166 alpha-2, e.g. US, GB, DE)"
            )
        })
}

/// Split a structured value on an unescaped `sep` (a backslash escapes the
/// separator, per vCard TEXT rules). Preserves empty components.
fn split_unescaped(value: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            cur.push(c);
            if let Some(n) = chars.next() {
                cur.push(n);
            }
        } else if c == sep {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    out.push(cur);
    out
}

/// RFC 6350 §3.2 line unfolding: normalize newlines, then a line beginning with a
/// single space or tab continues the previous line (that one leading whitespace
/// char is removed).
fn unfold(input: &str) -> Vec<String> {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut out: Vec<String> = Vec::new();
    for raw in normalized.split('\n') {
        if let Some(rest) = raw.strip_prefix(' ').or_else(|| raw.strip_prefix('\t')) {
            if let Some(last) = out.last_mut() {
                last.push_str(rest);
                continue;
            }
        }
        out.push(raw.to_string());
    }
    out
}

/// Find the first ':' not inside a double-quoted param value.
fn find_unquoted_colon(s: &str) -> Option<usize> {
    let mut in_quote = false;
    for (i, c) in s.char_indices() {
        match c {
            '"' => in_quote = !in_quote,
            ':' if !in_quote => return Some(i),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn norm(input: &str) -> String {
        run(input, "US", NameCase::Keep, true).unwrap()
    }

    #[test]
    fn lowercases_and_trims_email() {
        let vcf = "BEGIN:VCARD\nVERSION:3.0\nFN:Jane\nEMAIL;TYPE=work:  Jane.DOE@Example.COM \nEND:VCARD\n";
        let out = norm(vcf);
        assert!(
            out.contains("EMAIL;TYPE=work:jane.doe@example.com"),
            "email should be trimmed + lowercased:\n{out}"
        );
    }

    #[test]
    fn email_case_can_be_preserved() {
        let vcf = "BEGIN:VCARD\nVERSION:3.0\nEMAIL:Jane.DOE@Example.COM\nEND:VCARD\n";
        let out = run(vcf, "", NameCase::Keep, false).unwrap();
        assert!(out.contains("EMAIL:Jane.DOE@Example.COM"), "got:\n{out}");
    }

    #[test]
    fn reformats_phone_to_e164_with_region() {
        let vcf = "BEGIN:VCARD\nVERSION:3.0\nTEL;TYPE=CELL:(415) 555-2671\nEND:VCARD\n";
        let out = norm(vcf);
        assert!(
            out.contains("TEL;TYPE=CELL:+14155552671"),
            "phone should be E.164:\n{out}"
        );
    }

    #[test]
    fn keeps_extension_on_phone() {
        let vcf = "BEGIN:VCARD\nVERSION:3.0\nTEL:415 555 2671 x89\nEND:VCARD\n";
        let out = norm(vcf);
        assert!(
            out.contains("TEL:+14155552671;ext=89"),
            "extension should be preserved:\n{out}"
        );
    }

    #[test]
    fn plus_prefixed_number_needs_no_region() {
        let vcf = "BEGIN:VCARD\nVERSION:4.0\nTEL:+44 20 7183 8750\nEND:VCARD\n";
        let out = run(vcf, "", NameCase::Keep, true).unwrap();
        assert!(out.contains("TEL:+442071838750"), "got:\n{out}");
    }

    #[test]
    fn invalid_phone_is_left_untouched() {
        // A short/invalid number must NOT be mangled into a guessed E.164.
        let vcf = "BEGIN:VCARD\nVERSION:3.0\nTEL:12345\nEND:VCARD\n";
        let out = norm(vcf);
        assert!(out.contains("TEL:12345"), "invalid phone kept as-is:\n{out}");
    }

    #[test]
    fn title_cases_names_and_tidies_spacing() {
        let vcf = "BEGIN:VCARD\nVERSION:3.0\nFN:jOHN   doe\nN:DOE;john;;;\nNICKNAME:johnny , j\nEND:VCARD\n";
        let out = run(vcf, "US", NameCase::Title, true).unwrap();
        assert!(out.contains("FN:John Doe"), "FN title-cased + tidied:\n{out}");
        assert!(out.contains("N:Doe;John;;;"), "N structure preserved:\n{out}");
        assert!(out.contains("NICKNAME:Johnny,J"), "nickname tidied:\n{out}");
    }

    #[test]
    fn keep_case_only_tidies_whitespace() {
        let vcf = "BEGIN:VCARD\nVERSION:3.0\nFN:jOHN   doe\nEND:VCARD\n";
        let out = norm(vcf);
        assert!(out.contains("FN:jOHN doe"), "case kept, spaces collapsed:\n{out}");
    }

    #[test]
    fn preserves_unknown_properties_and_structure() {
        let vcf = "BEGIN:VCARD\nVERSION:3.0\nFN:A\nX-CUSTOM;FOO=bar:Keep Me VERBATIM\nORG:Acme, Inc.\nEND:VCARD\n";
        let out = norm(vcf);
        assert!(out.contains("X-CUSTOM;FOO=bar:Keep Me VERBATIM"), "unknown kept:\n{out}");
        assert!(out.contains("ORG:Acme, Inc."), "ORG kept:\n{out}");
        assert!(out.contains("VERSION:3.0"), "version kept:\n{out}");
    }

    #[test]
    fn preserves_crlf_line_endings() {
        let vcf = "BEGIN:VCARD\r\nVERSION:3.0\r\nEMAIL:A@B.COM\r\nEND:VCARD\r\n";
        let out = norm(vcf);
        assert!(out.contains("\r\n"), "CRLF preserved:\n{out:?}");
        assert!(out.contains("EMAIL:a@b.com\r\n"), "got:\n{out:?}");
    }

    #[test]
    fn unfolds_folded_lines() {
        let vcf = "BEGIN:VCARD\nVERSION:3.0\nEMAIL:LONG.ADDRESS\n @Example.COM\nEND:VCARD\n";
        let out = norm(vcf);
        assert!(out.contains("EMAIL:long.address@example.com"), "unfolded:\n{out}");
    }

    #[test]
    fn handles_multiple_cards() {
        let vcf = "BEGIN:VCARD\nVERSION:3.0\nEMAIL:A@X.COM\nEND:VCARD\nBEGIN:VCARD\nVERSION:3.0\nEMAIL:B@Y.COM\nEND:VCARD\n";
        let out = norm(vcf);
        assert_eq!(out.matches("BEGIN:VCARD").count(), 2, "two cards:\n{out}");
        assert!(out.contains("EMAIL:a@x.com"));
        assert!(out.contains("EMAIL:b@y.com"));
    }

    #[test]
    fn error_no_vcard() {
        let err = run("not a vcard at all", "", NameCase::Keep, true).unwrap_err();
        assert!(err.contains("no vCard found"), "got: {err}");
    }

    #[test]
    fn error_bad_region_code() {
        let vcf = "BEGIN:VCARD\nVERSION:3.0\nEND:VCARD\n";
        let err = run(vcf, "ZZ-not-a-country", NameCase::Keep, true).unwrap_err();
        assert!(err.contains("default_country"), "got: {err}");
    }

    #[test]
    fn error_bad_name_case() {
        let err = NameCase::parse("sentence").unwrap_err();
        assert!(err.contains("name_case"), "got: {err}");
    }
}
