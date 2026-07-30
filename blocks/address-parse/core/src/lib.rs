//! gizza-ai/address-parse core — split a freeform postal address string into
//! structured fields: house number, street, unit, city, region/state, postcode,
//! and country (with ISO codes where recognized). Pure-Rust, rule-based (no ML
//! model, no I/O). Shared by the chat block, the CLI and the web page.
//!
//! The parser is a heuristic tuned for the common comma-separated and
//! multi-line address formats. It is NOT a statistical model — unusual orderings
//! or country-specific quirks may parse imperfectly (see the page's stated
//! limits). Nothing is uploaded; parsing is entirely local.

use regex::Regex;
use serde::Serialize;

/// The structured result of parsing a freeform address. Every field is optional
/// because a freeform address may omit any component; fields that could not be
/// identified are simply left out of the JSON.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ParsedAddress {
    /// The primary/house number (e.g. `123`, `221B`, `12-14`), if the street
    /// line begins (or, for some countries, ends) with one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub house_number: Option<String>,
    /// The street/road line with the house number and any unit removed
    /// (e.g. `Main St`, `Baker Street`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street: Option<String>,
    /// A secondary unit designator kept verbatim (e.g. `Apt 4B`, `Suite 200`,
    /// `#12`), when a unit keyword is present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// The city / town / locality.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    /// The region — state / province / county — as written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// The region's canonical code (US/CA/AU only, e.g. `IL`, `ON`, `NSW`),
    /// when the region resolves to a known subdivision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region_code: Option<String>,
    /// The postal code / ZIP, normalized (UK/CA/NL uppercased with a single
    /// space; US ZIP+4 kept as `12345-6789`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postcode: Option<String>,
    /// The country name, canonicalized (e.g. `United States`, `United Kingdom`),
    /// either detected in the text or filled from the country hint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    /// The country's ISO 3166-1 alpha-2 code (e.g. `US`, `GB`), when recognized.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
}

/// The supported country hints (ISO 3166-1 alpha-2). `auto` = detect from text.
pub const COUNTRY_HINTS: [&str; 13] = [
    "auto", "US", "GB", "CA", "AU", "DE", "FR", "IN", "NL", "ES", "IT", "BR", "JP",
];

/// (aliases lowercased, ISO alpha-2 code, canonical display name).
const COUNTRIES: &[(&[&str], &str, &str)] = &[
    (
        &[
            "united states", "united states of america", "usa", "us", "u.s.a.", "u.s.",
            "america",
        ],
        "US",
        "United States",
    ),
    (
        &[
            "united kingdom", "uk", "u.k.", "great britain", "britain", "england",
            "scotland", "wales", "northern ireland",
        ],
        "GB",
        "United Kingdom",
    ),
    (&["canada"], "CA", "Canada"),
    (&["australia"], "AU", "Australia"),
    (&["germany", "deutschland"], "DE", "Germany"),
    (&["france"], "FR", "France"),
    (&["india"], "IN", "India"),
    (&["netherlands", "the netherlands", "holland"], "NL", "Netherlands"),
    (&["spain", "espana", "españa"], "ES", "Spain"),
    (&["italy", "italia"], "IT", "Italy"),
    (&["brazil", "brasil"], "BR", "Brazil"),
    (&["japan"], "JP", "Japan"),
];

/// US states + DC: (code, name).
const US_STATES: &[(&str, &str)] = &[
    ("AL", "Alabama"), ("AK", "Alaska"), ("AZ", "Arizona"), ("AR", "Arkansas"),
    ("CA", "California"), ("CO", "Colorado"), ("CT", "Connecticut"), ("DE", "Delaware"),
    ("FL", "Florida"), ("GA", "Georgia"), ("HI", "Hawaii"), ("ID", "Idaho"),
    ("IL", "Illinois"), ("IN", "Indiana"), ("IA", "Iowa"), ("KS", "Kansas"),
    ("KY", "Kentucky"), ("LA", "Louisiana"), ("ME", "Maine"), ("MD", "Maryland"),
    ("MA", "Massachusetts"), ("MI", "Michigan"), ("MN", "Minnesota"), ("MS", "Mississippi"),
    ("MO", "Missouri"), ("MT", "Montana"), ("NE", "Nebraska"), ("NV", "Nevada"),
    ("NH", "New Hampshire"), ("NJ", "New Jersey"), ("NM", "New Mexico"), ("NY", "New York"),
    ("NC", "North Carolina"), ("ND", "North Dakota"), ("OH", "Ohio"), ("OK", "Oklahoma"),
    ("OR", "Oregon"), ("PA", "Pennsylvania"), ("RI", "Rhode Island"), ("SC", "South Carolina"),
    ("SD", "South Dakota"), ("TN", "Tennessee"), ("TX", "Texas"), ("UT", "Utah"),
    ("VT", "Vermont"), ("VA", "Virginia"), ("WA", "Washington"), ("WV", "West Virginia"),
    ("WI", "Wisconsin"), ("WY", "Wyoming"), ("DC", "District of Columbia"),
];

/// Canadian provinces/territories: (code, name).
const CA_PROVINCES: &[(&str, &str)] = &[
    ("AB", "Alberta"), ("BC", "British Columbia"), ("MB", "Manitoba"), ("NB", "New Brunswick"),
    ("NL", "Newfoundland and Labrador"), ("NS", "Nova Scotia"), ("NT", "Northwest Territories"),
    ("NU", "Nunavut"), ("ON", "Ontario"), ("PE", "Prince Edward Island"), ("QC", "Quebec"),
    ("SK", "Saskatchewan"), ("YT", "Yukon"),
];

/// Australian states/territories: (code, name).
const AU_STATES: &[(&str, &str)] = &[
    ("ACT", "Australian Capital Territory"), ("NSW", "New South Wales"),
    ("NT", "Northern Territory"), ("QLD", "Queensland"), ("SA", "South Australia"),
    ("TAS", "Tasmania"), ("VIC", "Victoria"), ("WA", "Western Australia"),
];

/// Unit / secondary-designator keywords (lowercased, without trailing dots).
const UNIT_KEYWORDS: &[&str] = &[
    "apt", "apartment", "suite", "ste", "unit", "flat", "fl", "floor", "rm", "room",
    "bldg", "building", "no", "number", "dept", "department", "lot", "trlr", "space", "spc",
];

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Split into trimmed, non-empty parts on commas and line breaks.
fn split_parts(input: &str) -> Vec<String> {
    input
        .split(|c| c == ',' || c == '\n' || c == '\r' || c == ';')
        .map(collapse_ws)
        .filter(|p| !p.is_empty())
        .collect()
}

/// If `code` names a known country, return its canonical display name.
fn country_name_for_code(code: &str) -> Option<&'static str> {
    let up = code.to_ascii_uppercase();
    COUNTRIES
        .iter()
        .find(|(_, c, _)| *c == up)
        .map(|(_, _, name)| *name)
}

/// Try to strip a country from the END of the last part's words. Returns
/// `(code, name, leftover_part)` when matched.
fn match_country_suffix(part: &str) -> Option<(&'static str, &'static str, String)> {
    let words: Vec<&str> = part.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }
    // Try the longest suffix first (up to 4 words, e.g. "united states of america").
    let max = words.len().min(4);
    for n in (1..=max).rev() {
        let start = words.len() - n;
        let candidate = words[start..].join(" ").to_ascii_lowercase();
        for (aliases, code, name) in COUNTRIES {
            if aliases.contains(&candidate.as_str()) {
                let leftover = words[..start].join(" ");
                return Some((code, name, leftover));
            }
        }
    }
    None
}

/// Build the postcode regex(es) to try for a given country code. When the code
/// is unknown, a general set is returned (tried in order).
fn postcode_patterns(code: Option<&str>) -> Vec<Regex> {
    let mk = |p: &str| Regex::new(p).expect("valid postcode regex");
    match code {
        Some("US") => vec![mk(r"\b\d{5}(?:-\d{4})?\b")],
        Some("GB") => vec![mk(r"(?i)\b[A-Z]{1,2}\d[A-Z\d]?\s*\d[A-Z]{2}\b")],
        Some("CA") => vec![mk(r"(?i)\b[A-Z]\d[A-Z]\s*\d[A-Z]\d\b")],
        Some("NL") => vec![mk(r"(?i)\b\d{4}\s*[A-Z]{2}\b")],
        Some("AU") => vec![mk(r"\b\d{4}\b")],
        Some("IN") => vec![mk(r"\b\d{6}\b")],
        Some("JP") => vec![mk(r"\b\d{3}-?\d{4}\b")],
        Some("BR") => vec![mk(r"\b\d{5}-?\d{3}\b")],
        Some("DE") | Some("FR") | Some("ES") | Some("IT") => vec![mk(r"\b\d{5}\b")],
        // Unknown country: try the most distinctive formats first, then digits.
        _ => vec![
            mk(r"(?i)\b[A-Z]{1,2}\d[A-Z\d]?\s+\d[A-Z]{2}\b"), // UK (require a space to avoid false hits)
            mk(r"(?i)\b[A-Z]\d[A-Z]\s+\d[A-Z]\d\b"),          // CA (require a space)
            mk(r"\b\d{5}(?:-\d{4})?\b"),                       // US ZIP / ZIP+4
            mk(r"\b\d{4,6}\b"),                                // generic 4–6 digit
        ],
    }
}

/// Normalize a matched postcode: uppercase and collapse internal whitespace to a
/// single space for alphanumeric (UK/CA/NL) codes.
fn normalize_postcode(raw: &str) -> String {
    let up = raw.trim().to_ascii_uppercase();
    if up.chars().any(|c| c.is_ascii_alphabetic()) {
        up.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        up
    }
}

/// Find a postcode across `parts`, scanning from the last part toward the first
/// (but never the first part unless it is the only one — a leading house number
/// must not be mistaken for a postcode). Returns `(part_index, normalized,
/// match_range_in_part)`.
fn find_postcode(parts: &[String], code: Option<&str>) -> Option<(usize, String, (usize, usize))> {
    let patterns = postcode_patterns(code);
    let lowest = if parts.len() == 1 { 0 } else { 1 };
    for idx in (lowest..parts.len()).rev() {
        for re in &patterns {
            if let Some(m) = re.find(&parts[idx]) {
                return Some((idx, normalize_postcode(m.as_str()), (m.start(), m.end())));
            }
        }
    }
    None
}

/// Try to identify a region (state/province) from the trailing words of `part`,
/// biased by the (possibly known) country code. Returns
/// `(region_name, region_code, leftover_part)`.
fn match_region_suffix(part: &str, code: Option<&str>) -> Option<(String, Option<String>, String)> {
    let words: Vec<&str> = part.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }

    // Which subdivision tables to consult, given the country.
    let tables: Vec<&[(&str, &str)]> = match code {
        Some("US") => vec![US_STATES],
        Some("CA") => vec![CA_PROVINCES],
        Some("AU") => vec![AU_STATES],
        // Unknown: try all three so a bare "IL"/"ON"/"NSW" still resolves.
        None => vec![US_STATES, CA_PROVINCES, AU_STATES],
        // Known non-subdivision country: no code table, region stays best-effort.
        _ => vec![],
    };

    // 1) Multi-word full name as a suffix (e.g. "New York", "British Columbia").
    let max = words.len().min(3);
    for n in (1..=max).rev() {
        let start = words.len() - n;
        let candidate = words[start..].join(" ");
        let cand_lc = candidate.to_ascii_lowercase();
        for table in &tables {
            for (c, name) in table.iter() {
                if name.to_ascii_lowercase() == cand_lc {
                    return Some((
                        (*name).to_string(),
                        Some((*c).to_string()),
                        words[..start].join(" "),
                    ));
                }
            }
        }
    }

    // 2) A bare subdivision CODE as the last word (e.g. "IL", "ON", "NSW").
    let last = words[words.len() - 1];
    let last_up = last.trim_end_matches('.').to_ascii_uppercase();
    for table in &tables {
        for (c, name) in table.iter() {
            if *c == last_up {
                return Some((
                    (*name).to_string(),
                    Some((*c).to_string()),
                    words[..words.len() - 1].join(" "),
                ));
            }
        }
    }
    None
}

/// Is `w` a plausible house number token (`123`, `221B`, `12-14`, `12-14A`)?
fn is_house_number(w: &str) -> bool {
    let re = Regex::new(r"^\d+(?:-\d+)?[A-Za-z]?$").expect("valid house-number regex");
    re.is_match(w)
}

/// Split a leading (or, for European ordering, trailing) house number off a
/// street line. Returns `(house_number, street_without_number)`.
fn split_house_number(street: &str, code: Option<&str>) -> (Option<String>, String) {
    let words: Vec<&str> = street.split_whitespace().collect();
    if words.is_empty() {
        return (None, String::new());
    }
    if is_house_number(words[0]) && words.len() > 1 {
        return (Some(words[0].to_string()), words[1..].join(" "));
    }
    // European ordering ("Hauptstraße 5"): number trails the street name.
    let trailing_number_countries =
        matches!(code, Some("DE") | Some("FR") | Some("ES") | Some("IT") | Some("NL"));
    if trailing_number_countries && words.len() > 1 && is_house_number(words[words.len() - 1]) {
        return (
            Some(words[words.len() - 1].to_string()),
            words[..words.len() - 1].join(" "),
        );
    }
    // A lone numeric token is a house number with no street name.
    if words.len() == 1 && is_house_number(words[0]) {
        return (Some(words[0].to_string()), String::new());
    }
    (None, street.to_string())
}

/// Split a unit/secondary designator off a street line. Returns
/// `(street_without_unit, unit)`.
fn split_unit(street: &str) -> (String, Option<String>) {
    let words: Vec<&str> = street.split_whitespace().collect();
    for (i, w) in words.iter().enumerate() {
        let key = w.trim_end_matches('.').to_ascii_lowercase();
        let is_hash = w.starts_with('#');
        if is_hash || UNIT_KEYWORDS.contains(&key.as_str()) {
            // Don't treat the very first word as a unit — that's the street start.
            if i == 0 && !is_hash {
                continue;
            }
            let street_part = words[..i].join(" ");
            let unit = words[i..].join(" ");
            let street_part = street_part.trim().to_string();
            if street_part.is_empty() {
                // Unit keyword began the line — keep it as the street to avoid
                // dropping data (rare; documented limit).
                return (street.to_string(), None);
            }
            return (street_part, Some(unit));
        }
    }
    (street.to_string(), None)
}

/// Parse a freeform address into structured fields. `country_hint` is an ISO
/// alpha-2 code or `auto`. Returns `Err` only for empty/whitespace-only input.
pub fn parse(input: &str, country_hint: &str) -> Result<ParsedAddress, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("empty input: provide a postal address to parse".into());
    }

    let hint = country_hint.trim();
    let hint_code: Option<&str> = if hint.is_empty() || hint.eq_ignore_ascii_case("auto") {
        None
    } else {
        let up = hint.to_ascii_uppercase();
        // Validate against the supported set; an unknown hint is treated as auto.
        COUNTRIES
            .iter()
            .find(|(_, c, _)| *c == up)
            .map(|(_, c, _)| *c)
    };

    let mut parts = split_parts(trimmed);
    let mut out = ParsedAddress::default();

    // 1) Country — try to strip it from the last part's trailing words.
    if let Some(last) = parts.last().cloned() {
        if let Some((code, name, leftover)) = match_country_suffix(&last) {
            out.country = Some(name.to_string());
            out.country_code = Some(code.to_string());
            let idx = parts.len() - 1;
            if leftover.trim().is_empty() {
                parts.remove(idx);
            } else {
                parts[idx] = leftover;
            }
        }
    }
    // Fill country from the hint when the text did not name one.
    if out.country_code.is_none() {
        if let Some(hc) = hint_code {
            out.country_code = Some(hc.to_string());
            out.country = country_name_for_code(hc).map(str::to_string);
        }
    }

    // Effective country code for postcode/region biasing.
    let eff_code: Option<String> =
        out.country_code.clone().or_else(|| hint_code.map(str::to_string));
    let eff_code_ref = eff_code.as_deref();

    // 2) Postcode.
    if let Some((idx, normalized, (s, e))) = find_postcode(&parts, eff_code_ref) {
        out.postcode = Some(normalized);
        let mut leftover = String::new();
        leftover.push_str(parts[idx][..s].trim());
        let tail = parts[idx][e..].trim();
        if !tail.is_empty() {
            if !leftover.is_empty() {
                leftover.push(' ');
            }
            leftover.push_str(tail);
        }
        let leftover = collapse_ws(&leftover);
        if leftover.is_empty() {
            parts.remove(idx);
        } else {
            parts[idx] = leftover;
        }
    }

    // 3) Region — check the last non-empty part's trailing words.
    if let Some(last_idx) = parts.iter().rposition(|p| !p.is_empty()) {
        // Skip treating the first part as a region unless it is the only one.
        if last_idx > 0 || parts.len() == 1 {
            if let Some((name, code, leftover)) = match_region_suffix(&parts[last_idx], eff_code_ref)
            {
                out.region = Some(name);
                out.region_code = code;
                let leftover = collapse_ws(&leftover);
                if leftover.is_empty() {
                    parts.remove(last_idx);
                } else {
                    parts[last_idx] = leftover;
                }
            }
        }
    }

    // 4) Street / city from what remains.
    let remaining: Vec<String> = parts.into_iter().filter(|p| !p.is_empty()).collect();
    let street_line: Option<String> = match remaining.len() {
        0 => None,
        1 => {
            let only = remaining[0].clone();
            // If the sole remaining part looks like a street (leading house
            // number) OR nothing else was extracted, treat it as the street.
            // Otherwise (a region/postcode/country was found) it is more likely
            // the city.
            let has_house = split_house_number(&only, eff_code_ref).0.is_some();
            let something_else = out.region.is_some() || out.postcode.is_some();
            if has_house || !something_else {
                Some(only)
            } else {
                out.city = Some(only);
                None
            }
        }
        _ => {
            // First part is the street line; the last is the city; any middle
            // parts are additional street/locality lines folded into the street.
            let city = remaining[remaining.len() - 1].clone();
            out.city = Some(city);
            Some(remaining[..remaining.len() - 1].join(", "))
        }
    };

    if let Some(line) = street_line {
        let (line_no_unit, unit) = split_unit(&line);
        out.unit = unit;
        let (house, street) = split_house_number(&line_no_unit, eff_code_ref);
        out.house_number = house;
        let street = collapse_ws(&street);
        if !street.is_empty() {
            out.street = Some(street);
        }
    }

    Ok(out)
}

/// Parse and return as pretty JSON (chat / programmatic surface).
pub fn run(input: &str, country_hint: &str) -> Result<String, String> {
    let parsed = parse(input, country_hint)?;
    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

/// Human-readable rendering (used by the page).
pub fn render(input: &str, country_hint: &str) -> Result<String, String> {
    let p = parse(input, country_hint)?;
    let mut out = String::new();
    let mut row = |label: &str, val: &str| {
        out.push_str(&format!("{label:<14}{val}\n"));
    };
    if let Some(v) = &p.house_number {
        row("House number:", v);
    }
    if let Some(v) = &p.street {
        row("Street:", v);
    }
    if let Some(v) = &p.unit {
        row("Unit:", v);
    }
    if let Some(v) = &p.city {
        row("City:", v);
    }
    if let Some(v) = &p.region {
        let line = match &p.region_code {
            Some(c) if c != v => format!("{v} ({c})"),
            _ => v.clone(),
        };
        row("Region:", &line);
    }
    if let Some(v) = &p.postcode {
        row("Postcode:", v);
    }
    if let Some(v) = &p.country {
        let line = match &p.country_code {
            Some(c) => format!("{v} ({c})"),
            None => v.clone(),
        };
        row("Country:", &line);
    }
    if out.is_empty() {
        out.push_str("(no address components could be identified)\n");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_us_address() {
        let p = parse("123 Main St, Springfield, IL 62704, USA", "auto").unwrap();
        assert_eq!(p.house_number.as_deref(), Some("123"));
        assert_eq!(p.street.as_deref(), Some("Main St"));
        assert_eq!(p.city.as_deref(), Some("Springfield"));
        assert_eq!(p.region.as_deref(), Some("Illinois"));
        assert_eq!(p.region_code.as_deref(), Some("IL"));
        assert_eq!(p.postcode.as_deref(), Some("62704"));
        assert_eq!(p.country.as_deref(), Some("United States"));
        assert_eq!(p.country_code.as_deref(), Some("US"));
    }

    #[test]
    fn parses_zip_plus_four() {
        let p = parse("1600 Pennsylvania Ave NW, Washington, DC 20500-0003", "auto").unwrap();
        assert_eq!(p.house_number.as_deref(), Some("1600"));
        assert_eq!(p.postcode.as_deref(), Some("20500-0003"));
        assert_eq!(p.region_code.as_deref(), Some("DC"));
        assert_eq!(p.city.as_deref(), Some("Washington"));
    }

    #[test]
    fn parses_uk_address_with_alphanumeric_postcode() {
        let p = parse("221B Baker Street, London, NW1 6XE, United Kingdom", "auto").unwrap();
        assert_eq!(p.house_number.as_deref(), Some("221B"));
        assert_eq!(p.street.as_deref(), Some("Baker Street"));
        assert_eq!(p.city.as_deref(), Some("London"));
        assert_eq!(p.postcode.as_deref(), Some("NW1 6XE"));
        assert_eq!(p.country_code.as_deref(), Some("GB"));
        // No US/CA/AU subdivision — region stays unset.
        assert_eq!(p.region, None);
    }

    #[test]
    fn parses_canadian_postal_code_and_province() {
        let p = parse("100 Queen St W, Toronto, ON M5H 2N2, Canada", "auto").unwrap();
        assert_eq!(p.postcode.as_deref(), Some("M5H 2N2"));
        assert_eq!(p.region.as_deref(), Some("Ontario"));
        assert_eq!(p.region_code.as_deref(), Some("ON"));
        assert_eq!(p.city.as_deref(), Some("Toronto"));
        assert_eq!(p.country_code.as_deref(), Some("CA"));
    }

    #[test]
    fn detects_unit_designator() {
        let p = parse("500 Market St Apt 4B, San Francisco, CA 94105", "auto").unwrap();
        assert_eq!(p.house_number.as_deref(), Some("500"));
        assert_eq!(p.street.as_deref(), Some("Market St"));
        assert_eq!(p.unit.as_deref(), Some("Apt 4B"));
        assert_eq!(p.region_code.as_deref(), Some("CA"));
    }

    #[test]
    fn hash_unit_designator() {
        let p = parse("742 Evergreen Terrace #12, Portland, OR 97201", "auto").unwrap();
        assert_eq!(p.unit.as_deref(), Some("#12"));
        assert_eq!(p.street.as_deref(), Some("Evergreen Terrace"));
    }

    #[test]
    fn multi_line_input_is_parsed() {
        let p = parse("10 Downing Street\nLondon\nSW1A 2AA\nUK", "auto").unwrap();
        assert_eq!(p.house_number.as_deref(), Some("10"));
        assert_eq!(p.street.as_deref(), Some("Downing Street"));
        assert_eq!(p.city.as_deref(), Some("London"));
        assert_eq!(p.postcode.as_deref(), Some("SW1A 2AA"));
        assert_eq!(p.country_code.as_deref(), Some("GB"));
    }

    #[test]
    fn country_hint_fills_missing_country() {
        // No country in the text; the hint supplies it and biases postcode.
        let p = parse("350 Fifth Avenue, New York, NY 10118", "US").unwrap();
        assert_eq!(p.country.as_deref(), Some("United States"));
        assert_eq!(p.country_code.as_deref(), Some("US"));
        assert_eq!(p.region.as_deref(), Some("New York"));
        assert_eq!(p.region_code.as_deref(), Some("NY"));
        assert_eq!(p.postcode.as_deref(), Some("10118"));
    }

    #[test]
    fn german_trailing_house_number() {
        let p = parse("Hauptstraße 5, 10115 Berlin, Germany", "auto").unwrap();
        assert_eq!(p.house_number.as_deref(), Some("5"));
        assert_eq!(p.street.as_deref(), Some("Hauptstraße"));
        assert_eq!(p.postcode.as_deref(), Some("10115"));
        assert_eq!(p.country_code.as_deref(), Some("DE"));
        // "Berlin" is left as the city after the postcode is stripped.
        assert_eq!(p.city.as_deref(), Some("Berlin"));
    }

    #[test]
    fn full_state_name_resolves_to_code() {
        let p =
            parse("1 Infinite Loop, Cupertino, California 95014, United States", "auto").unwrap();
        assert_eq!(p.region.as_deref(), Some("California"));
        assert_eq!(p.region_code.as_deref(), Some("CA"));
        assert_eq!(p.city.as_deref(), Some("Cupertino"));
    }

    #[test]
    fn city_and_state_without_street() {
        let p = parse("Springfield, IL 62704", "auto").unwrap();
        assert_eq!(p.city.as_deref(), Some("Springfield"));
        assert_eq!(p.region_code.as_deref(), Some("IL"));
        assert_eq!(p.postcode.as_deref(), Some("62704"));
        assert_eq!(p.street, None);
        assert_eq!(p.house_number, None);
    }

    #[test]
    fn australian_state_abbreviation() {
        let p = parse("42 Wallaby Way, Sydney NSW 2000, Australia", "auto").unwrap();
        assert_eq!(p.region.as_deref(), Some("New South Wales"));
        assert_eq!(p.region_code.as_deref(), Some("NSW"));
        assert_eq!(p.postcode.as_deref(), Some("2000"));
        assert_eq!(p.city.as_deref(), Some("Sydney"));
        assert_eq!(p.country_code.as_deref(), Some("AU"));
    }

    #[test]
    fn house_number_with_letter_suffix() {
        let p = parse("221B Baker Street", "GB").unwrap();
        assert_eq!(p.house_number.as_deref(), Some("221B"));
        assert_eq!(p.street.as_deref(), Some("Baker Street"));
    }

    #[test]
    fn hyphenated_house_number() {
        let p = parse("12-14 High Street, Oxford, OX1 4AB, UK", "auto").unwrap();
        assert_eq!(p.house_number.as_deref(), Some("12-14"));
        assert_eq!(p.street.as_deref(), Some("High Street"));
    }

    #[test]
    fn unknown_hint_treated_as_auto() {
        let p = parse("123 Main St, Springfield, IL 62704, USA", "ZZ").unwrap();
        assert_eq!(p.country_code.as_deref(), Some("US"));
    }

    #[test]
    fn rejects_empty_input() {
        assert!(parse("", "auto").is_err());
        assert!(parse("   \n  ", "auto").is_err());
    }

    #[test]
    fn run_emits_valid_json() {
        let j = run("123 Main St, Springfield, IL 62704, USA", "auto").unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["house_number"], "123");
        assert_eq!(v["region_code"], "IL");
        assert_eq!(v["country_code"], "US");
    }

    #[test]
    fn render_is_human_readable() {
        let out = render("123 Main St, Springfield, IL 62704, USA", "auto").unwrap();
        assert!(out.contains("House number:"));
        assert!(out.contains("Street:"));
        assert!(out.contains("Illinois (IL)"));
        assert!(out.contains("United States (US)"));
    }

    #[test]
    fn omitted_fields_are_not_serialized() {
        let j = run("Baker Street", "GB").unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert!(v.get("postcode").is_none());
        assert!(v.get("city").is_none());
        // Country came from the hint.
        assert_eq!(v["country_code"], "GB");
    }
}
