//! phone-format core — parse, validate, and format an international phone number.
//!
//! No wafer/wasm-bindgen deps. Shared by the chat skill block and the web page.
//! Backed by the pure-Rust `phonenumber` crate (bundles libphonenumber metadata,
//! so the compiled wasm is larger than a typical pure tool — acceptable per spec).

use phonenumber::country;
use phonenumber::Mode;

/// Parse, validate, and format a phone `number`.
///
/// - `number` may carry its own country prefix (`+1 415 555 2671`). When it does
///   not, `region` (an ISO-3166 alpha-2 code such as `US`) is used to interpret it.
/// - `region` may be empty (the common case for `+`-prefixed numbers). When empty
///   and the number lacks a `+` prefix, parsing fails with a helpful message.
///
/// Returns a human-readable multi-line report: validity, E.164, national &
/// international formats, the country/region, and the number type when derivable.
/// Returns `Err` with a readable message when the input can't be parsed.
pub fn format_number(number: &str, region: &str) -> Result<String, String> {
    let number = number.trim();
    if number.is_empty() {
        return Err("no phone number provided".into());
    }

    // Resolve the optional region hint. An invalid code is reported rather than
    // silently ignored, so the caller knows the hint was rejected.
    let region_id = parse_region(region)?;

    let parsed = phonenumber::parse(region_id, number).map_err(|e| {
        if region_id.is_none() && !number.starts_with('+') {
            format!(
                "could not parse {number:?}: {e}. Provide the number in international form \
                 (e.g. +1 415 555 2671) or pass a region (e.g. region=US)."
            )
        } else {
            format!("could not parse {number:?}: {e}")
        }
    })?;

    let valid = phonenumber::is_valid(&parsed);
    let e164 = parsed.format().mode(Mode::E164).to_string();
    let national = parsed.format().mode(Mode::National).to_string();
    let international = parsed.format().mode(Mode::International).to_string();

    let region_label = parsed
        .country()
        .id()
        .map(|id| format!("{id:?}"))
        .unwrap_or_else(|| format!("calling code +{}", parsed.code().value()));

    let type_label = describe_type(number_type(&parsed));

    let mut out = String::new();
    out.push_str(if valid {
        "Valid: yes\n"
    } else {
        "Valid: no (parsed, but not a valid number for its region)\n"
    });
    out.push_str(&format!("E.164: {e164}\n"));
    out.push_str(&format!("National: {national}\n"));
    out.push_str(&format!("International: {international}\n"));
    out.push_str(&format!("Country/region: {region_label}\n"));
    out.push_str(&format!("Type: {type_label}"));
    Ok(out)
}

/// Parse an optional ISO-3166 alpha-2 region hint into a `country::Id`.
/// Empty/whitespace → `None`. An unrecognised code is an error.
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
                "unrecognised region code {region:?} (expected ISO-3166 alpha-2, e.g. US, GB, DE)"
            )
        })
}

/// The number's `Type` using the crate's bundled default metadata database.
fn number_type(parsed: &phonenumber::PhoneNumber) -> phonenumber::Type {
    parsed.number_type(&phonenumber::metadata::DATABASE)
}

/// Human-readable label for a phone number `Type`. Unknown types report as
/// "unknown" rather than a Debug dump.
fn describe_type(t: phonenumber::Type) -> &'static str {
    use phonenumber::Type::*;
    match t {
        FixedLine => "fixed line",
        Mobile => "mobile",
        FixedLineOrMobile => "fixed line or mobile",
        TollFree => "toll-free",
        PremiumRate => "premium rate",
        SharedCost => "shared cost",
        PersonalNumber => "personal number",
        Voip => "VoIP",
        Pager => "pager",
        Uan => "UAN",
        Emergency => "emergency",
        Voicemail => "voicemail",
        ShortCode => "short code",
        StandardRate => "standard rate",
        Carrier => "carrier-specific",
        NoInternational => "no international dialling",
        Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_international_us() {
        let r = format_number("+14155552671", "").unwrap();
        assert!(r.starts_with("Valid: yes"), "expected valid in:\n{r}");
        assert!(r.contains("E.164: +14155552671"), "e164 in:\n{r}");
        assert!(r.contains("Country/region: US"), "country in:\n{r}");
    }

    #[test]
    fn national_with_region() {
        let r = format_number("4155552671", "US").unwrap();
        assert!(r.starts_with("Valid: yes"), "expected valid in:\n{r}");
        assert!(r.contains("E.164: +14155552671"), "e164 in:\n{r}");
        assert!(r.contains("Country/region: US"), "country in:\n{r}");
    }

    #[test]
    fn region_is_case_insensitive() {
        let r = format_number("4155552671", "us").unwrap();
        assert!(r.contains("E.164: +14155552671"), "e164 in:\n{r}");
    }

    #[test]
    fn invalid_number_too_short() {
        // Parses as a US-region number but is not a valid US number.
        let r = format_number("12345", "US").unwrap();
        assert!(r.starts_with("Valid: no"), "expected not valid in:\n{r}");
    }

    #[test]
    fn national_without_region_errors() {
        let err = format_number("4155552671", "").unwrap_err();
        assert!(
            err.contains("region") || err.contains("international"),
            "expected a region/international hint in error: {err}"
        );
    }

    #[test]
    fn bad_region_code_errors() {
        let err = format_number("4155552671", "ZZ-not-a-country").unwrap_err();
        assert!(err.contains("region"), "expected region error: {err}");
    }

    #[test]
    fn empty_number_errors() {
        let err = format_number("   ", "").unwrap_err();
        assert!(err.contains("no phone number"), "got: {err}");
    }
}
