//! gizza-ai/mac-address-format core — reformat one or many MAC (EUI-48 / EUI-64)
//! addresses between the four common notations (colon, hyphen, Cisco dotted-quad,
//! and bare hex) with a chosen upper/lower hex case. Pure-Rust, no I/O.
//!
//! Unlike `extract-mac-addresses` (which scans free text and de-duplicates), this
//! tool treats its input as a list of addresses to reformat: every token is
//! validated, order is preserved, and duplicates are kept — so it is a 1:1
//! re-styler, not a finder.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    /// Number of addresses reformatted.
    pub count: usize,
    /// The reformatted addresses, in input order (duplicates preserved).
    pub macs: Vec<String>,
}

/// Output notation.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    /// `00:1a:2b:3c:4d:5e` (IEEE 802 canonical, colon-separated).
    Colon,
    /// `00-1a-2b-3c-4d-5e` (Windows / IEEE hyphen form).
    Hyphen,
    /// `001a.2b3c.4d5e` (Cisco dotted-quad; EUI-64 → four groups).
    Cisco,
    /// `001a2b3c4d5e` (no separators).
    Bare,
}

impl Format {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "colon" => Ok(Format::Colon),
            "hyphen" | "dash" => Ok(Format::Hyphen),
            "cisco" | "dot" | "dotted" => Ok(Format::Cisco),
            "bare" | "none" | "plain" => Ok(Format::Bare),
            other => Err(format!(
                "invalid format {other:?}: expected \"colon\", \"hyphen\", \"cisco\", or \"bare\""
            )),
        }
    }
}

/// Output hex case.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Case {
    Lower,
    Upper,
}

impl Case {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "lower" | "lowercase" => Ok(Case::Lower),
            "upper" | "uppercase" => Ok(Case::Upper),
            other => Err(format!(
                "invalid case {other:?}: expected \"lower\" or \"upper\""
            )),
        }
    }
}

/// Strip the separators (`:`, `-`, `.`) from a single token and validate that the
/// remainder is a valid MAC: exactly 12 (EUI-48) or 16 (EUI-64) hex digits.
/// Returns the lowercase hex string on success.
fn canonical_hex(token: &str) -> Result<String, String> {
    let t = token.trim();
    // Reject any character that isn't a hex digit or an accepted separator so a
    // log line / sentence isn't silently mangled — this tool expects addresses.
    if let Some(bad) = t
        .chars()
        .find(|c| !(c.is_ascii_hexdigit() || matches!(c, ':' | '-' | '.')))
    {
        return Err(format!(
            "{token:?} is not a MAC address (unexpected character {bad:?})"
        ));
    }
    let hex: String = t
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    match hex.len() {
        12 | 16 => Ok(hex),
        n => Err(format!(
            "{token:?} is not a MAC address ({n} hex digits; expected 12 for EUI-48 or 16 for EUI-64)"
        )),
    }
}

/// Render a 12/16-char lowercase hex string into the requested notation + case.
fn render_one(hex: &str, fmt: Format, case: Case) -> String {
    let grouped = match fmt {
        Format::Bare => hex.to_string(),
        Format::Colon | Format::Hyphen => {
            let sep = if fmt == Format::Colon { ":" } else { "-" };
            (0..hex.len())
                .step_by(2)
                .map(|i| &hex[i..i + 2])
                .collect::<Vec<_>>()
                .join(sep)
        }
        Format::Cisco => (0..hex.len())
            .step_by(4)
            .map(|i| &hex[i..i + 4])
            .collect::<Vec<_>>()
            .join("."),
    };
    match case {
        Case::Lower => grouped,
        Case::Upper => grouped.to_ascii_uppercase(),
    }
}

/// Reformat every MAC address in `input` to the chosen `format` + `case`.
///
/// Addresses are separated by any whitespace, comma, or semicolon, so a list one
/// per line and a single space- or comma-separated line both work. Order is
/// preserved and duplicates are kept. Returns `Err` on an invalid option, on no
/// addresses at all, or on the first token that is not a valid MAC address.
pub fn reformat(input: &str, format: &str, case: &str) -> Result<Output, String> {
    let fmt = Format::parse(format)?;
    let cs = Case::parse(case)?;

    let tokens: Vec<&str> = input
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .filter(|t| !t.is_empty())
        .collect();
    if tokens.is_empty() {
        return Err("no MAC addresses supplied".to_string());
    }

    let mut macs = Vec::with_capacity(tokens.len());
    for tok in tokens {
        let hex = canonical_hex(tok)?;
        macs.push(render_one(&hex, fmt, cs));
    }
    Ok(Output { count: macs.len(), macs })
}

/// Human-readable rendering (used by the page): one reformatted address per line.
pub fn render(input: &str, format: &str, case: &str) -> Result<String, String> {
    let out = reformat(input, format, case)?;
    Ok(out.macs.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reformats_colon_to_each_format() {
        let src = "AA:BB:CC:DD:EE:FF";
        assert_eq!(reformat(src, "colon", "lower").unwrap().macs, vec!["aa:bb:cc:dd:ee:ff"]);
        assert_eq!(reformat(src, "hyphen", "lower").unwrap().macs, vec!["aa-bb-cc-dd-ee-ff"]);
        assert_eq!(reformat(src, "cisco", "lower").unwrap().macs, vec!["aabb.ccdd.eeff"]);
        assert_eq!(reformat(src, "bare", "lower").unwrap().macs, vec!["aabbccddeeff"]);
    }

    #[test]
    fn honors_upper_case() {
        let r = reformat("00:1a:2b:3c:4d:5e", "colon", "upper").unwrap();
        assert_eq!(r.macs, vec!["00:1A:2B:3C:4D:5E"]);
        let r = reformat("00:1a:2b:3c:4d:5e", "cisco", "upper").unwrap();
        assert_eq!(r.macs, vec!["001A.2B3C.4D5E"]);
    }

    #[test]
    fn accepts_any_input_notation() {
        // hyphen, cisco, and bare inputs all normalize the same way.
        for src in ["00-1a-2b-3c-4d-5e", "001a.2b3c.4d5e", "001A2B3C4D5E"] {
            let r = reformat(src, "colon", "lower").unwrap();
            assert_eq!(r.macs, vec!["00:1a:2b:3c:4d:5e"], "input {src:?}");
        }
    }

    #[test]
    fn reformats_many_preserving_order_and_dups() {
        let src = "11:22:33:44:55:66, aabbccddeeff\n11:22:33:44:55:66";
        let r = reformat(src, "hyphen", "lower").unwrap();
        assert_eq!(
            r.macs,
            vec!["11-22-33-44-55-66", "aa-bb-cc-dd-ee-ff", "11-22-33-44-55-66"]
        );
        assert_eq!(r.count, 3);
    }

    #[test]
    fn handles_eui64() {
        // 8-byte EUI-64 → four Cisco groups / eight colon groups.
        let r = reformat("00:1a:2b:ff:fe:3c:4d:5e", "cisco", "lower").unwrap();
        assert_eq!(r.macs, vec!["001a.2bff.fe3c.4d5e"]);
        let r = reformat("001a2bfffe3c4d5e", "colon", "lower").unwrap();
        assert_eq!(r.macs, vec!["00:1a:2b:ff:fe:3c:4d:5e"]);
    }

    #[test]
    fn rejects_invalid_width() {
        let err = reformat("00:11:22:33:44", "colon", "lower").unwrap_err();
        assert!(err.contains("hex digits"), "got: {err}");
    }

    #[test]
    fn rejects_non_hex_character() {
        let err = reformat("00:1a:2b:3c:4d:zz", "colon", "lower").unwrap_err();
        assert!(err.contains("unexpected character"), "got: {err}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = reformat("   \n  ", "colon", "lower").unwrap_err();
        assert!(err.contains("no MAC addresses"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_options() {
        assert!(reformat("aa:bb:cc:dd:ee:ff", "binary", "lower")
            .unwrap_err()
            .contains("invalid format"));
        assert!(reformat("aa:bb:cc:dd:ee:ff", "colon", "title")
            .unwrap_err()
            .contains("invalid case"));
    }

    #[test]
    fn render_joins_lines() {
        let out = render("aa:bb:cc:dd:ee:ff ffeeddccbbaa", "bare", "upper").unwrap();
        assert_eq!(out, "AABBCCDDEEFF\nFFEEDDCCBBAA");
    }
}
